use std::str::FromStr;

use super::{Cpu, Storage, StackType, Memory, Environment};

impl FromStr for Cpu {
	type Err = ();

	fn from_str(input: &str) -> core::result::Result<Cpu, Self::Err> {
			match input {
					"1"  => Ok(Cpu::Variant1),
					"2"  => Ok(Cpu::Variant2),
					"4"  => Ok(Cpu::Variant4),
					"8" => Ok(Cpu::Variant8),
					"16" => Ok(Cpu::Variant16),
					"32" => Ok(Cpu::Variant32),
					_      => Err(()),
			}
	}
}

impl FromStr for Memory {
	type Err = ();

	fn from_str(input: &str) -> core::result::Result<Memory, Self::Err> {
			match input {
					"1Gi"  => Ok(Memory::Variant1Gi),
					"2Gi"  => Ok(Memory::Variant2Gi),
					"4Gi"  => Ok(Memory::Variant4Gi),
					"8Gi" => Ok(Memory::Variant8Gi),
					"16Gi" => Ok(Memory::Variant16Gi),
					"32Gi" => Ok(Memory::Variant32Gi),
					_      => Err(()),
			}
	}
}

impl FromStr for Environment {
	type Err = ();

	fn from_str(input: &str) -> core::result::Result<Environment, Self::Err> {
			match input {
					"dev"  => Ok(Environment::Dev),
					"test"  => Ok(Environment::Test),
					"prod"  => Ok(Environment::Prod),
					_      => Err(()),
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

impl ToString for StackType {
	fn to_string(&self) -> String {
			match self {
					Self::Standard => String::from("Standard"),
					Self::MessageQueue => String::from("MessageQueue"),
					Self::MachineLearning => String::from("MachineLearning"),
					Self::Olap => String::from("OLAP"),
					Self::Oltp => String::from("OLTP"),
					Self::VectorDb => String::from("VectorDB"),
					Self::DataWarehouse => String::from("DataWarehouse"),
			}
	}
}