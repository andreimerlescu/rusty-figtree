# figtree/src/sources

This directory contains all configuration source implementations for
figtree. Each file implements the Source trait defined in priority.rs
for one specific configuration origin.

## What A Source Is

A Source is anything that can answer the question "do you have a value
for this key?" and return it as a FigValue. Sources do not validate,
apply rules, or record history. They only fetch. All other responsibilities
belong to the Tree.

The Source trait is defined in priority.rs rather than here to keep the
dependency direction clean. Sources depend on priority, not the reverse.
This means priority.rs and its resolve() function can be tested with mock
sources without importing anything from this directory.

## PEMDAS Position Of Each Source

Sources are consulted in this fixed order by resolve() in priority.rs.
This order is invariant and cannot be changed at runtime.

    1. CliSource      — CLI flags (highest priority)
    2. EnvSource      — environment variables
    3. file sources   — in registration order (YamlSource, JsonSource,
                        TomlSource, IniSource, PlistSource, DotenvSource,
                        RonSource) — first registered wins
    4. EmbeddedSource — bare metal hardware reads
    5. default        — declared in Fig at registration time (lowest)

## Source Files

    mod.rs          Module root. Declares submodules with feature gates
                    and re-exports all source types at the sources:: level.

    env.rs          Reads from os::var. Always compiled — no feature gate.
                    Parses raw strings into typed FigValue using the hint's
                    mutagenesis variant. The parse_env_value() function is
                    also used by CliSource, IniSource, DotenvSource, and
                    EmbeddedSource since all are string-origin sources.

    cli.rs          Wraps a snapshot of clap ArgMatches taken at parse time.
                    Feature gated: "cli". Delegates string-to-FigValue
                    parsing to parse_env_value from env.rs.

    yaml.rs         Reads .yaml and .yml files via serde_yaml. Feature
                    gated: "yaml". File is parsed once at construction.
                    Handles the YAML document start marker (---) correctly
                    since serde_yaml processes it transparently. Both .yaml
                    and .yml extensions are accepted — the parser does not
                    care about the extension, only the content.

    json.rs         Reads .json files via serde_json. Feature gated: "json".
                    File parsed once at construction. Scalar JSON values
                    are converted to FigValue on demand via get_typed().

    toml.rs         Reads .toml files via the toml crate. Feature gated:
                    "toml". TOML is the de facto standard format in the
                    Rust ecosystem — Cargo itself uses it. Including TOML
                    support is non-negotiable for community acceptance.
                    TOML's typed nature (integers, floats, booleans, arrays,
                    and tables are distinct in the format) means conversion
                    to FigValue is more precise than string-origin sources.

    ini.rs          Reads .ini files via rust-ini. Feature gated: "ini".
                    Section-qualified keys are stored as "section.key" in
                    the flat internal map. All values are strings in INI
                    format — parse_env_value drives typed conversion.

    plist.rs        Reads Apple property list files in XML, binary, and
                    NeXTSTEP ASCII formats via the plist crate. Feature
                    gated: "plist". Useful for macOS and iOS applications,
                    Swift interop, and any engineer working in the Apple
                    ecosystem. Plist's typed nature (similar to TOML)
                    means integer, float, boolean, array, and dictionary
                    values map cleanly to FigValue variants without string
                    parsing.

    dotenv.rs       Reads .env files via​​​​​​​​​​​​​​​​
