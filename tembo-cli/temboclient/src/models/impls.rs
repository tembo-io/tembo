use std::str::FromStr;

use super::{Cpu, Environment, Memory, StackType, Storage};

impl FromStr for Cpu {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Cpu, Self::Err> {
        match input {
            "0.25" => Ok(Cpu::Variant0Period25),
            "0.5" => Ok(Cpu::Variant0Period5),
            "1" => Ok(Cpu::Variant1),
            "2" => Ok(Cpu::Variant2),
            "4" => Ok(Cpu::Variant4),
            "8" => Ok(Cpu::Variant8),
            "16" => Ok(Cpu::Variant16),
            "32" => Ok(Cpu::Variant32),
            _ => Err(()),
        }
    }
}

impl FromStr for Memory {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Memory, Self::Err> {
        match input {
            "1Gi" => Ok(Memory::Variant1Gi),
            "2Gi" => Ok(Memory::Variant2Gi),
            "4Gi" => Ok(Memory::Variant4Gi),
            "8Gi" => Ok(Memory::Variant8Gi),
            "16Gi" => Ok(Memory::Variant16Gi),
            "32Gi" => Ok(Memory::Variant32Gi),
            _ => Err(()),
        }
    }
}

impl FromStr for Environment {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Environment, Self::Err> {
        match input {
            "dev" => Ok(Environment::Dev),
            "test" => Ok(Environment::Test),
            "prod" => Ok(Environment::Prod),
            _ => Err(()),
        }
    }
}

impl FromStr for Storage {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Storage, Self::Err> {
        match input {
            "10Gi" => Ok(Storage::Variant10Gi),
            "50Gi" => Ok(Storage::Variant50Gi),
            "100Gi" => Ok(Storage::Variant100Gi),
            "200Gi" => Ok(Storage::Variant200Gi),
            "300Gi" => Ok(Storage::Variant300Gi),
            "400Gi" => Ok(Storage::Variant400Gi),
            "500Gi" => Ok(Self::Variant500Gi),
            _ => Err(()),
        }
    }
}

impl FromStr for StackType {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<StackType, Self::Err> {
        match input {
            "Standard" => Ok(StackType::Standard),
            "MessageQueue" => Ok(StackType::MessageQueue),
            "MachineLearning" => Ok(StackType::MachineLearning),
            "OLAP" => Ok(StackType::Olap),
            "VectorDB" => Ok(StackType::VectorDb),
            "OLTP" => Ok(StackType::Oltp),
            "Analytics" => Ok(StackType::Analytics),
            "DataWarehouse" => Ok(StackType::DataWarehouse),
            "Geospatial" => Ok(StackType::Geospatial),
            "MongoAlternative" => Ok(StackType::MongoAlternative),
            "RAG" => Ok(StackType::Rag),
            "Timeseries" => Ok(StackType::Timeseries),
            "ParadeDB" => Ok(StackType::ParadeDB),
            _ => Err(()),
        }
    }
}
