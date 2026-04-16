use mage_contract::{FlatCall, FlatExpression, FlatIndex, FlatRoot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapForm {
    Procedure,
    If,
    While,
    Return,
    Break,
    Continue,
}

impl BootstrapForm {
    pub fn name(self) -> &'static str {
        match self {
            Self::Procedure => "procedure",
            Self::If => "if",
            Self::While => "while",
            Self::Return => "return",
            Self::Break => "break",
            Self::Continue => "continue",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcedureDeclarationShape {
    pub parameter_source_index: usize,
    pub return_source_index: Option<usize>,
    pub body_source_index: usize,
}

pub fn classify_bootstrap_name(name: &str) -> Option<BootstrapForm> {
    match name {
        "procedure" => Some(BootstrapForm::Procedure),
        "if" => Some(BootstrapForm::If),
        "while" => Some(BootstrapForm::While),
        "return" => Some(BootstrapForm::Return),
        "break" => Some(BootstrapForm::Break),
        "continue" => Some(BootstrapForm::Continue),
        _ => None,
    }
}

pub fn classify_bootstrap_call(root: &FlatRoot, index: &FlatIndex) -> Option<BootstrapForm> {
    let FlatIndex::Expression(expression_index) = index else {
        return None;
    };
    let FlatExpression::Call(call) = root.get_expression(*expression_index) else {
        return None;
    };
    classify_bootstrap_call_expression(root, call)
}

pub fn classify_bootstrap_call_expression(
    root: &FlatRoot,
    call: &FlatCall,
) -> Option<BootstrapForm> {
    let FlatIndex::Identifier(name_index) = call.name else {
        return None;
    };
    classify_bootstrap_name(root.get_string(name_index))
}

pub fn recognize_procedure_declaration(
    root: &FlatRoot,
    index: &FlatIndex,
) -> Option<ProcedureDeclarationShape> {
    let FlatIndex::Expression(expression_index) = index else {
        return None;
    };
    let FlatExpression::Call(call) = root.get_expression(*expression_index) else {
        return None;
    };

    let mut chained_arguments = Vec::with_capacity(4);
    let root_name_index = collect_left_associated_call_chain(root, call, &mut chained_arguments)?;
    if classify_bootstrap_name(root.get_string(root_name_index)) != Some(BootstrapForm::Procedure) {
        return None;
    }

    let mut source_indices: Vec<usize> = chained_arguments
        .into_iter()
        .filter_map(|argument| match argument {
            FlatIndex::Source(source_index) => Some(source_index as usize),
            _ => None,
        })
        .collect();

    let parameter_source_index = *source_indices.first()?;
    source_indices.remove(0);
    let body_source_index = source_indices.pop()?;

    let return_source_index = match source_indices.len() {
        0 => None,
        1 => Some(source_indices[0]),
        _ => return None,
    };

    Some(ProcedureDeclarationShape {
        parameter_source_index,
        return_source_index,
        body_source_index,
    })
}

pub fn is_procedure_definition(root: &FlatRoot, index: &FlatIndex) -> bool {
    recognize_procedure_declaration(root, index).is_some()
}

fn collect_left_associated_call_chain(
    root: &FlatRoot,
    call: &FlatCall,
    arguments: &mut Vec<FlatIndex>,
) -> Option<u32> {
    let root_name_index = match call.name {
        FlatIndex::Identifier(name_index) => name_index,
        FlatIndex::Expression(callee_expression_index) => {
            let FlatExpression::Call(inner_call) = root.get_expression(callee_expression_index)
            else {
                return None;
            };
            collect_left_associated_call_chain(root, inner_call, arguments)?
        }
        _ => return None,
    };

    arguments.extend_from_slice(root.get_extra_indices(call.arguments_start, call.arguments_end));

    Some(root_name_index)
}
