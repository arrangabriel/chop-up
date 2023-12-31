use std::cmp::Ordering;
use std::fmt::{Display, Formatter};

use anyhow::{anyhow, Result};
use wast::core::Instruction::{
    self as WastInstruction, Block, Br, BrIf, Drop, End, F32Const, F32Gt, F64Const, F64Gt, I32Add,
    I32Const, I32Eq, I32Eqz, I32GtS, I32GtU, I32Load, I32Load16u, I32LtS, I32LtU, I32Mul, I32Ne,
    I32Shl, I32Store, I32Store8, I32Sub, I32WrapI64, I32Xor, I64Add, I64Const, I64Eq,
    I64ExtendI32U, I64GtS, I64GtU, I64Load, I64Load32u, I64LtS, I64LtU, I64Mul, I64Ne, I64Sub,
    I64Xor, LocalGet, LocalSet, LocalTee, Return,
};
use wast::token::Index;
use WastInstruction::{I32And, I32Store16, I64Store};

use crate::chop_up::instruction::{
    BenignInstructionType, BlockInstructionType, DataType, InstructionType,
};
use crate::chop_up::utils::UTX_LOCALS;

pub struct Instruction<'a> {
    pub instr: &'a WastInstruction<'a>,
    pub raw_text: String,
    pub index: usize,
    pub stack: Vec<StackValue>,
    pub scopes: Vec<Scope>,
}

impl<'a> Instruction<'a> {
    pub fn new(
        instr: &'a WastInstruction<'a>,
        raw_text: String,
        index: usize,
        stack: Vec<StackValue>,
        scopes: Vec<Scope>,
    ) -> Self {
        Instruction {
            instr,
            raw_text,
            index,
            stack,
            scopes,
        }
    }

    pub fn default(
        instr: &'a WastInstruction<'a>,
        raw_text: String,
    ) -> Self {
        Self::new(
            instr,
            raw_text,
            0,
            Vec::default(),
            Vec::default(),
        )
    }
}

#[derive(Clone)]
pub struct Scope {
    pub ty: ScopeType,
    pub name: Option<String>,
    pub stack_start: usize,
}

#[derive(Clone)]
pub enum ScopeType {
    Block,
}

#[derive(Copy, Clone, Debug)]
pub struct StackValue {
    pub ty: DataType,
    pub is_safe: bool,
}

impl Display for StackValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let safe_string = if self.is_safe { " - safe" } else { "" };
        write!(f, "({:?}{safe_string})", self.ty)
    }
}

pub enum StackEffect {
    Normal {
        remove_n: usize,
        add: Option<StackValue>,
        preserves_safety: bool,
    },
    Return,
}

impl StackEffect {
    fn new(remove_n: usize, add: Option<DataType>, is_safe: bool, preserves_safety: bool) -> Self {
        Self::Normal {
            remove_n,
            add: add.map(|ty| StackValue { ty, is_safe }),
            preserves_safety,
        }
    }

    pub fn update_stack(&self, stack: &mut Vec<StackValue>) -> Result<()> {
        let mut is_safe = false;
        match self {
            StackEffect::Normal {
                remove_n,
                add,
                preserves_safety,
            } => {
                for _ in 0..*remove_n {
                    let stack_value = stack
                        .pop()
                        .ok_or(anyhow!("Unbalanced stack - input program is malformed"))?;
                    is_safe |= *preserves_safety && *remove_n == 1 && stack_value.is_safe;
                }
                if let Some(mut stack_value) = add {
                    stack_value.is_safe |= is_safe;
                    stack.push(stack_value);
                }
            }
            StackEffect::Return => stack.clear(),
        }
        Ok(())
    }

    // IMPORTANT!
    // This is the only place where we can detect unsupported instructions.
    // When adding a memory instruction it should also be added to the implementation of
    // InstructionType::from<&(Wast)Instruction>.
    pub fn from_wast_instruction(instruction: &WastInstruction, local_types: &[DataType]) -> Self {
        match instruction {
            Return => Self::Return,
            End(_) | Block(_) | Br(_) => Self::new(0, None, false, false),
            LocalGet(index) => {
                let (ty, is_safe) = type_and_safety_from_param(index, local_types);
                Self::new(0, Some(ty), is_safe, true)
            }
            LocalTee(_) => Self::new(0, None, false, false),
            I64Load(_) | I64Load32u(_) | I64ExtendI32U => {
                Self::new(1, Some(DataType::I64), false, false)
            }
            I64Const(_) => Self::new(0, Some(DataType::I64), false, false),
            I32WrapI64 | I32Load(_) | I32Load16u(_) | I32Eqz => {
                Self::new(1, Some(DataType::I32), false, true)
            }
            I32Const(_) => Self::new(0, Some(DataType::I32), false, false),
            I32Mul | I32Add | I32Sub | I32Eq | F64Gt | F32Gt | I32GtU | I32GtS | I64GtU
            | I64GtS | I32LtU | I32LtS | I64LtU | I64LtS | I64Eq | I32Ne | I64Ne | I32Shl
            | I32Xor | I32And => Self::new(2, Some(DataType::I32), false, false),
            I64Mul | I64Add | I64Xor | I64Sub => Self::new(2, Some(DataType::I64), false, false),
            I32Store(_) | I32Store8(_) | I32Store16(_) | I64Store(_) => {
                Self::new(2, None, false, false)
            }
            Drop | BrIf(_) | LocalSet(_) => Self::new(1, None, false, false),
            F64Const(_) => Self::new(0, Some(DataType::F64), false, false),
            F32Const(_) => Self::new(0, Some(DataType::F32), false, false),
            _ => panic!(
                "Unsupported instruction read when producing StackEffect - {:?}",
                instruction
            ),
        }
    }

    pub fn from_instruction(instruction: &Instruction, local_types: &[DataType]) -> Self {
        Self::from_wast_instruction(instruction.instr, local_types)
    }
}

fn type_and_safety_from_param(index: &Index, local_types: &[DataType]) -> (DataType, bool) {
    match index {
        Index::Num(index, _) => {
            let index = *index;
            let safe = index_is_param(index);
            let mut utx_locals = Vec::default();
            utx_locals.extend_from_slice(&UTX_LOCALS);
            utx_locals.extend_from_slice(local_types);
            let ty = *utx_locals
                .get(index as usize)
                .expect("Indexed get to locals should use in bounds index");
            (ty, safe)
        }
        // TODO - to be completely safe we need to check the type of id'd locals
        // in case compiled code uses them (usually not)
        Index::Id(id) => (DataType::I32, name_is_param(id.name())),
    }
}

fn name_is_param(name: &str) -> bool {
    matches!(name, "tx" | "state")
}

/// Assuming use in a function of the type (tx, state) -> ?
pub fn index_is_param(index: u32) -> bool {
    index < 3
}

/// To be used at some point inside of a scope
pub fn index_of_scope_end(instructions: &[Instruction]) -> Result<usize> {
    let mut scope_level = 1;
    for (i, instruction_with_text) in instructions.iter().enumerate() {
        if let InstructionType::Benign(BenignInstructionType::Block(block_instruction_type)) =
            InstructionType::from(instruction_with_text)
        {
            scope_level += match block_instruction_type {
                BlockInstructionType::End => -1,
                BlockInstructionType::Block(_) => 1,
            };

            match scope_level.cmp(&0) {
                Ordering::Equal => return Ok(i),
                Ordering::Less => return Err(anyhow!("Unbalanced scope delimiters")),
                Ordering::Greater => {}
            }
        }
    }
    Err(anyhow!("Unbalanced scope delimiters"))
}
