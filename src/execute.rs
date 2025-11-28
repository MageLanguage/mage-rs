use mmap_rs::{MmapFlags, MmapOptions, UnsafeMmapFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::CStr;
use std::mem;
use std::os::raw::c_char;

use crate::{Bytecode, Error, FlatRoot, Mage, Stage, Type, compile_lazy};

#[repr(C)]
struct Procedure {
    code: usize,
    source_index: usize,
    root: usize,
}

#[repr(C)]
struct Coroutine {
    _registers: [usize; 8],
}

#[repr(C)]
struct Runtime {
    _args_ptr: usize,
    _args_len: usize,
    variables: *mut HashMap<String, usize>,
    set_var: usize,
    get_var: usize,
    export_table: *mut ExportTable,
    export_var: usize,
    mage: usize,
    import: usize,
    get_member: usize,
    set_member: usize,
    make_procedure: usize,
    compile_procedure: usize,
    push_scope: usize,
    pop_scope: usize,
    make_syscall_procedure: usize,
    root: usize,
    scope_stack: *mut Vec<*mut HashMap<String, usize>>,
    modules: *mut HashMap<String, usize>,
}

#[repr(C)]
struct Arg {
    _pointer: usize,
    _length: usize,
}

#[repr(C)]
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub interface_type: InterfaceType,
    pub interface_data: usize,
}

#[repr(usize)]
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum InterfaceType {
    Void,
    Number,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ExportTable {
    pub values: HashMap<String, usize>,
}

extern "sysv64" fn runtime_set_var(runtime: &mut Runtime, name_ptr: *const c_char, value: usize) {
    unsafe {
        let name = CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
        (*runtime.variables).insert(name, value);
    }
}

extern "sysv64" fn runtime_get_var(runtime: &mut Runtime, name_ptr: *const c_char) -> usize {
    unsafe {
        let name = CStr::from_ptr(name_ptr).to_string_lossy();
        *(*runtime.variables).get(name.as_ref()).unwrap_or(&0)
    }
}

extern "sysv64" fn runtime_export_var(
    runtime: &mut Runtime,
    name_ptr: *const c_char,
    value: usize,
) {
    unsafe {
        let name = CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
        (*runtime.export_table).values.insert(name, value);
    }
}

extern "sysv64" fn runtime_import(runtime: &mut Runtime, path_ptr: *const c_char) -> usize {
    unsafe {
        let modules = &mut *runtime.modules;
        let mut path = CStr::from_ptr(path_ptr).to_string_lossy().into_owned();

        if let Some(&module) = modules.get(&path) {
            return module;
        }

        if !path.ends_with(".mg") {
            path.push_str(".mg");
        }

        if let Some(&module) = modules.get(&path) {
            return module;
        }

        let mut content = std::fs::read_to_string(&path);
        if content.is_err() {
            let alt_path = format!("../mage/{}", path);
            content = std::fs::read_to_string(&alt_path);
        }
        let content = content.unwrap_or_default();

        let mage = &mut *(runtime.mage as *mut Mage);
        if let Ok(Type::Bytecode(bytecode)) = mage.process(&Stage::Compile, &content) {
            if let Ok(export_table) = execute_bytecode(bytecode, mage, modules) {
                let ptr = Box::into_raw(Box::new(export_table)) as usize;
                modules.insert(path, ptr);
                return ptr;
            }
        }
        0
    }
}

extern "sysv64" fn runtime_get_member(
    _runtime: &mut Runtime,
    table_ptr: usize,
    name_ptr: *const c_char,
) -> usize {
    unsafe {
        let table = &*(table_ptr as *const ExportTable);
        let name = CStr::from_ptr(name_ptr).to_string_lossy();
        *table.values.get(name.as_ref()).unwrap_or(&0)
    }
}

extern "sysv64" fn runtime_set_member(
    _runtime: &mut Runtime,
    table_ptr: usize,
    name_ptr: *const c_char,
    value: usize,
) {
    unsafe {
        let table = &mut *(table_ptr as *mut ExportTable);
        let name = CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
        table.values.insert(name, value);
    }
}

extern "sysv64" fn runtime_make_procedure(runtime: &mut Runtime, source_index: usize) -> usize {
    let procedure = Box::new(Procedure {
        code: 0,
        source_index,
        root: runtime.root,
    });
    Box::into_raw(procedure) as usize
}

extern "sysv64" fn runtime_make_syscall_procedure(
    _runtime: &mut Runtime,
    code_ptr: usize,
) -> usize {
    let procedure = Box::new(Procedure {
        code: code_ptr,
        source_index: 0,
        root: 0,
    });
    Box::into_raw(procedure) as usize
}

extern "sysv64" fn runtime_compile_procedure(
    _runtime: &mut Runtime,
    procedure_ptr: usize,
) -> usize {
    unsafe {
        let procedure = &mut *(procedure_ptr as *mut Procedure);
        if procedure.code != 0 {
            return procedure.code;
        }

        let root = &*(procedure.root as *const FlatRoot);
        if let Ok(code) = compile_lazy(root, procedure.source_index) {
            if let Ok(mut map) = MmapOptions::new(code.len()).map_err(|_| ()).and_then(|m| {
                m.with_unsafe_flags(UnsafeMmapFlags::JIT)
                    .map_exec_mut()
                    .map_err(|_| ())
            }) {
                map.copy_from_slice(code.as_slice());
                let ptr = map.as_ptr() as usize;
                std::mem::forget(map); // Leaking memory map to keep code alive
                procedure.code = ptr;
                return ptr;
            }
        }
        0
    }
}

extern "sysv64" fn runtime_push_scope(runtime: &mut Runtime) {
    unsafe {
        let stack = &mut *runtime.scope_stack;
        stack.push(runtime.variables);
        let new_scope = Box::new(HashMap::new());
        runtime.variables = Box::into_raw(new_scope);
    }
}

extern "sysv64" fn runtime_pop_scope(runtime: &mut Runtime) -> usize {
    unsafe {
        let scope = runtime.variables;
        let stack = &mut *runtime.scope_stack;
        if let Some(prev) = stack.pop() {
            runtime.variables = prev;
        }
        let table = Box::new(ExportTable {
            values: *Box::from_raw(scope),
        });
        Box::into_raw(table) as usize
    }
}

pub fn execute_bytecode(
    bytecode: Bytecode,
    mage: &mut Mage,
    modules: &mut HashMap<String, usize>,
) -> Result<ExportTable, Error> {
    unsafe {
        let root = Box::into_raw(Box::new(bytecode.root));

        let mut executable_map = MmapOptions::new(bytecode.code.len())
            .map_err(|error| {
                Error::ExecuteError(format!("Failed to create memory map: {}", error))
            })?
            .with_unsafe_flags(UnsafeMmapFlags::JIT)
            .map_exec_mut()
            .map_err(|error| Error::ExecuteError(format!("Failed to map memory: {}", error)))?;

        executable_map.copy_from_slice(bytecode.code.as_slice());

        let stack_map = MmapOptions::new(64 * 1024)
            .map_err(|error| {
                Error::ExecuteError(format!("Failed to create memory map: {}", error))
            })?
            .with_flags(MmapFlags::STACK)
            .map_mut()
            .map_err(|error| Error::ExecuteError(format!("Failed to map memory: {}", error)))?;

        let stack_end = stack_map.size() - 8;
        let stack_ptr = stack_map.as_ptr().add(stack_end) as *mut usize;

        *stack_ptr = executable_map.as_ptr().add(bytecode.main) as usize;

        let old = Coroutine { _registers: [0; 8] };
        let new = Coroutine {
            _registers: [0, 0, 0, 0, 0, 0, 0, stack_ptr as usize],
        };

        let call = mem::transmute::<
            *const u8,
            extern "sysv64" fn(
                old: &Coroutine,
                new: &Coroutine,
                runtime: &mut Runtime,
                export_table: &mut ExportTable,
            ),
        >(executable_map.as_ptr());

        let args: Vec<String> = std::env::args().collect();
        let args_converted: Vec<Arg> = args
            .iter()
            .map(|arg| Arg {
                _pointer: arg.as_ptr() as usize,
                _length: arg.len(),
            })
            .collect();

        let mut variables: HashMap<String, usize> = HashMap::new();
        let mut export_table = ExportTable {
            values: HashMap::new(),
        };

        let mut scope_stack: Vec<*mut HashMap<String, usize>> = Vec::new();

        let mut runtime = Runtime {
            _args_ptr: args_converted.as_ptr() as usize,
            _args_len: args_converted.len(),
            variables: &mut variables as *mut _,
            set_var: runtime_set_var as *const () as usize,
            get_var: runtime_get_var as *const () as usize,
            export_table: &mut export_table as *mut _,
            export_var: runtime_export_var as *const () as usize,
            mage: mage as *mut _ as usize,
            import: runtime_import as *const () as usize,
            get_member: runtime_get_member as *const () as usize,
            set_member: runtime_set_member as *const () as usize,
            make_procedure: runtime_make_procedure as *const () as usize,
            compile_procedure: runtime_compile_procedure as *const () as usize,
            push_scope: runtime_push_scope as *const () as usize,
            pop_scope: runtime_pop_scope as *const () as usize,
            make_syscall_procedure: runtime_make_syscall_procedure as *const () as usize,
            root: root as usize,
            scope_stack: &mut scope_stack as *mut _,
            modules: modules as *mut _,
        };

        call(&old, &new, &mut runtime, &mut export_table);

        Ok(export_table)
    }
}
