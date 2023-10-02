use wast::parser::ParseBuffer;
use wast::{parser, Wat};

use ast_parsing::get_module_data_from_ast;
use ast_parsing::pretty_print_ast;

use crate::module_analysis::print_accessors;

mod module_data;

mod ast_parsing;
mod module_analysis;

pub fn parse_wast_string(wast_string: &str, print_ast: bool) -> Result<(), wast::Error> {
    let buffer = ParseBuffer::new(wast_string)?;
    let wat = parser::parse::<Wat>(&buffer)?;
    let module = get_module_data_from_ast(&wat);

    if print_ast {
        pretty_print_ast(&wat);
        println!();
    }

    print_accessors(module.functions.as_ref());

    Ok(())
}
