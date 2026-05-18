# rusty-figtree
A rust 🦀 port of the figtree 🐿️ written in Go. 

```
figtree/
├── Cargo.toml
├── VERSION
├── build.rs
├── README.md
├── LICENSE
├── CHANGELOG.md
├── .gitignore
└── src/
    ├── lib.rs              # crate root — re-exports, feature gates
    ├── error.rs            # FigtreeError enum
    ├── tree.rs             # Tree struct — the core grow/new/with logic
    ├── fig.rs              # Fig struct — individual config value container
    ├── mutation.rs         # Mutation struct + MutationReceiver
    ├── priority.rs         # PEMDAS resolution logic
    ├── rules.rs            # RulePreventChange, RulePanicOnChange, etc.
    ├── validators.rs       # all Assure<Type><Rule> functions
    ├── callbacks.rs        # CallbackAfterVerify/Read/Change types
    ├── sources/
    │   ├── mod.rs          # Source trait definition
    │   ├── env.rs          # environment variable source
    │   ├── cli.rs          # clap integration (feature = "cli")
    │   ├── yaml.rs         # serde_yaml (feature = "yaml")
    │   ├── json.rs         # serde_json (feature = "json")
    │   ├── ini.rs          # ini parser (feature = "ini")
    │   └── embedded.rs     # EEPROM/flash (feature = "embedded")
    └── tests/
        ├── tree_test.rs
        ├── validators_test.rs
        ├── priority_test.rs
        ├── sources_test.rs
        └── callbacks_test.rs
```


