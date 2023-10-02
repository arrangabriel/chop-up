use wast::core::{Func, FunctionType, Instruction, ValType};
use wast::Wat;

use crate::ast::{walk_ast, AstWalker};

#[derive(Debug)]
pub struct Module<'a> {
    functions: Box<[Function<'a>]>,
}

#[derive(Debug)]
pub struct Function<'a> {
    id: Option<String>,
    signature: Signature<'a>,
    instructions: &'a [Instruction<'a>],
}

#[derive(Debug)]
pub struct Signature<'a> {
    parameters: Vec<FuncParameter<'a>>,
    results: Vec<ValType<'a>>,
}

#[derive(Debug)]
pub struct FuncParameter<'a> {
    id: Option<&'a str>,
    val_type: &'a ValType<'a>,
}

pub fn parse_module_struct_from_ast<'a>(ast: &'a Wat<'a>) -> Module<'a> {
    let extractor = Box::new(AstModuleStructExtractor::new());
    walk_ast(ast, extractor)
}

struct AstModuleStructExtractor<'a> {
    functions: Option<Vec<Function<'a>>>,
    current_func_id: Option<String>,
    current_func_signature: Option<Signature<'a>>,
}

impl AstModuleStructExtractor<'_> {
    fn new() -> Self {
        AstModuleStructExtractor {
            functions: None,
            current_func_id: None,
            current_func_signature: None,
        }
    }
}

impl<'a> AstWalker<'a> for AstModuleStructExtractor<'a> {
    type WalkResult = Module<'a>;

    fn start_handle_func(&mut self, func: &Func) {
        self.current_func_id = func.id.map(|id| {
            let mut id_string = String::new();
            id.name().clone_into(&mut id_string);
            id_string
        });
    }

    fn handle_func_type(&mut self, func_type: &'a FunctionType) {
        let params: Vec<FuncParameter> = func_type
            .params
            .iter()
            .map(|(id, _, val_type)| FuncParameter {
                val_type,
                id: id.map(|id| id.name()),
            })
            .collect();

        self.current_func_signature = Some(Signature {
            parameters: params,
            results: func_type.results.to_vec(),
        })
    }

    fn handle_func_instructions(&mut self, instructions: &'a [Instruction]) {
        let id = self.current_func_id.take();
        if let Some(signature) = self.current_func_signature.take() {
            let new_func = Function {
                id,
                signature,
                instructions,
            };
            let mut functions = match self.functions.take() {
                None => Vec::new(),
                Some(current_functions) => current_functions,
            };
            functions.push(new_func);
            self.functions = Some(functions);
        }
    }

    fn finish_and_build_result(&mut self) -> Self::WalkResult {
        let functions = match self.functions.take() {
            None => Vec::new(),
            Some(functions) => functions,
        }
        .into_boxed_slice();

        Module { functions }
    }
}