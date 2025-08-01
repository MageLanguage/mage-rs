use crate::Bytecode;

pub fn execute_bytecode(bytecode: Bytecode) -> i64 {
    unsafe {
        let call: fn() -> i64 = std::mem::transmute(bytecode);
        call()
    }
}
