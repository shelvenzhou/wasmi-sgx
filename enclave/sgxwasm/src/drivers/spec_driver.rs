use std::collections::HashMap;
use std::prelude::v1::*;
use wasmi::memory_units::Pages;

pub use wasmi::Error as InterpreterError;
use wasmi::{
    Externals,
    FuncInstance,
    FuncRef,
    GlobalDescriptor,
    GlobalInstance,
    GlobalRef,
    ImportResolver,
    MemoryDescriptor,
    // NopExternals,
    MemoryInstance,
    MemoryRef,
    ModuleImportResolver,
    ModuleRef,
    RuntimeArgs,
    RuntimeValue,
    Signature,
    TableDescriptor,
    TableInstance,
    TableRef,
    Trap,
};

use wabt::script;

pub struct SpecModule {
    table: TableRef,
    memory: MemoryRef,
    global_i32: GlobalRef,
    global_f32: GlobalRef,
    global_f64: GlobalRef,
}

impl SpecModule {
    pub fn new() -> Self {
        SpecModule {
            table: TableInstance::alloc(10, Some(20)).unwrap(),
            memory: MemoryInstance::alloc(Pages(1), Some(Pages(2))).unwrap(),
            global_i32: GlobalInstance::alloc(RuntimeValue::I32(666), false),
            global_f32: GlobalInstance::alloc(RuntimeValue::F32(666.0.into()), false),
            global_f64: GlobalInstance::alloc(RuntimeValue::F64(666.0.into()), false),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Load(String),
    Start(Trap),
    Script(script::Error),
    Interpreter(InterpreterError),
}

impl From<InterpreterError> for Error {
    fn from(e: InterpreterError) -> Error {
        Error::Interpreter(e)
    }
}

impl From<script::Error> for Error {
    fn from(e: script::Error) -> Error {
        Error::Script(e)
    }
}

const PRINT_FUNC_INDEX: usize = 0;

impl Externals for SpecModule {
    fn invoke_index(
        &mut self,
        index: usize,
        args: RuntimeArgs,
    ) -> Result<Option<RuntimeValue>, Trap> {
        match index {
            PRINT_FUNC_INDEX => {
                println!("print: {:?}", args);
                Ok(None)
            }
            _ => panic!("SpecModule doesn't provide function at index {}", index),
        }
    }
}

impl ModuleImportResolver for SpecModule {
    fn resolve_func(
        &self,
        field_name: &str,
        func_type: &Signature,
    ) -> Result<FuncRef, InterpreterError> {
        let index = match field_name {
            "print" => PRINT_FUNC_INDEX,
            "print_i32" => PRINT_FUNC_INDEX,
            "print_i32_f32" => PRINT_FUNC_INDEX,
            "print_f64_f64" => PRINT_FUNC_INDEX,
            "print_f32" => PRINT_FUNC_INDEX,
            "print_f64" => PRINT_FUNC_INDEX,
            _ => {
                return Err(InterpreterError::Instantiation(format!(
                    "Unknown host func import {}",
                    field_name
                )));
            }
        };

        if func_type.return_type().is_some() {
            return Err(InterpreterError::Instantiation(
                "Function `print_` have unit return type".into(),
            ));
        }

        let func = FuncInstance::alloc_host(func_type.clone(), index);
        return Ok(func);
    }
    fn resolve_global(
        &self,
        field_name: &str,
        _global_type: &GlobalDescriptor,
    ) -> Result<GlobalRef, InterpreterError> {
        match field_name {
            "global_i32" => Ok(self.global_i32.clone()),
            "global_f32" => Ok(self.global_f32.clone()),
            "global_f64" => Ok(self.global_f64.clone()),
            _ => Err(InterpreterError::Instantiation(format!(
                "Unknown host global import {}",
                field_name
            ))),
        }
    }

    fn resolve_memory(
        &self,
        field_name: &str,
        _memory_type: &MemoryDescriptor,
    ) -> Result<MemoryRef, InterpreterError> {
        if field_name == "memory" {
            return Ok(self.memory.clone());
        }

        Err(InterpreterError::Instantiation(format!(
            "Unknown host memory import {}",
            field_name
        )))
    }

    fn resolve_table(
        &self,
        field_name: &str,
        _table_type: &TableDescriptor,
    ) -> Result<TableRef, InterpreterError> {
        if field_name == "table" {
            return Ok(self.table.clone());
        }

        Err(InterpreterError::Instantiation(format!(
            "Unknown host table import {}",
            field_name
        )))
    }
}

pub struct SpecDriver {
    spec_module: SpecModule,
    instances: HashMap<String, ModuleRef>,
    last_module: Option<ModuleRef>,
}

impl SpecDriver {
    pub fn new() -> SpecDriver {
        SpecDriver {
            spec_module: SpecModule::new(),
            instances: HashMap::new(),
            last_module: None,
        }
    }

    pub fn externals(&mut self) -> &mut SpecModule {
        &mut self.spec_module
    }

    pub fn add_module(&mut self, name: Option<String>, module: ModuleRef) {
        self.last_module = Some(module.clone());
        if let Some(name) = name {
            self.instances.insert(name, module);
        }
    }

    pub fn module(&self, name: &str) -> Result<ModuleRef, InterpreterError> {
        self.instances.get(name).cloned().ok_or_else(|| {
            InterpreterError::Instantiation(format!("Module not registered {}", name))
        })
    }

    pub fn module_or_last(&self, name: Option<&str>) -> Result<ModuleRef, InterpreterError> {
        match name {
            Some(name) => self.module(name),
            None => self
                .last_module
                .clone()
                .ok_or_else(|| InterpreterError::Instantiation("No modules registered".into())),
        }
    }

    pub fn register(
        &mut self,
        name: &Option<String>,
        as_name: String,
    ) -> Result<(), InterpreterError> {
        let module = match self.module_or_last(name.as_ref().map(|x| x.as_ref())) {
            Ok(module) => module,
            Err(_) => {
                return Err(InterpreterError::Instantiation(
                    "No such modules registered".into(),
                ))
            }
        };
        self.add_module(Some(as_name), module);
        Ok(())
    }
}

impl ImportResolver for SpecDriver {
    fn resolve_func(
        &self,
        module_name: &str,
        field_name: &str,
        func_type: &Signature,
    ) -> Result<FuncRef, InterpreterError> {
        if module_name == "spectest" {
            self.spec_module.resolve_func(field_name, func_type)
        } else {
            self.module(module_name)?
                .resolve_func(field_name, func_type)
        }
    }

    fn resolve_global(
        &self,
        module_name: &str,
        field_name: &str,
        global_type: &GlobalDescriptor,
    ) -> Result<GlobalRef, InterpreterError> {
        if module_name == "spectest" {
            self.spec_module.resolve_global(field_name, global_type)
        } else {
            self.module(module_name)?
                .resolve_global(field_name, global_type)
        }
    }

    fn resolve_memory(
        &self,
        module_name: &str,
        field_name: &str,
        memory_type: &MemoryDescriptor,
    ) -> Result<MemoryRef, InterpreterError> {
        if module_name == "spectest" {
            self.spec_module.resolve_memory(field_name, memory_type)
        } else {
            self.module(module_name)?
                .resolve_memory(field_name, memory_type)
        }
    }

    fn resolve_table(
        &self,
        module_name: &str,
        field_name: &str,
        table_type: &TableDescriptor,
    ) -> Result<TableRef, InterpreterError> {
        if module_name == "spectest" {
            self.spec_module.resolve_table(field_name, table_type)
        } else {
            self.module(module_name)?
                .resolve_table(field_name, table_type)
        }
    }
}
