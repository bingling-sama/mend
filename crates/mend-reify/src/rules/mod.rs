pub mod cargo;
pub mod git;
pub mod node;
pub mod python;
pub mod system;

use mend_core::RuleRegistry;

pub fn build_default_rule_registry() -> RuleRegistry {
    let mut registry = RuleRegistry::new();

    // 1. System & Permissions
    registry.register(Box::new(system::SudoRule));
    registry.register(Box::new(system::MkdirRule));

    // 2. Node & Web package managers
    registry.register(Box::new(node::PnpmMissingRunRule));
    registry.register(Box::new(node::NpmMissingRunRule));
    registry.register(Box::new(node::YarnMissingRunRule));

    // 3. Git operations
    registry.register(Box::new(git::GitSetUpstreamRule));
    registry.register(Box::new(git::GitPullRebaseRule));
    registry.register(Box::new(git::GitSubcommandTypoRule));

    // 4. Rust / Cargo
    registry.register(Box::new(cargo::CargoSubcommandTypoRule));

    // 5. Python / Pip
    registry.register(Box::new(python::PythonMissingModuleRule));

    registry
}
