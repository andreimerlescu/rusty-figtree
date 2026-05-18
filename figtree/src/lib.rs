pub mod error;
pub mod rules;
pub mod fig;
pub mod mutation;
pub mod callbacks;
pub mod validators;
pub mod priority;

pub use error::{FigtreeError, FigtreeResult};
pub use rules::Rule;
pub use fig::{Fig, FigValue, FigSource, FigState, FigHistoryEntry};
pub use mutation::{Mutation, MutationSender, MutationReceiver, mutation_channel};
pub use callbacks::{Callback, CallbackPhase, CallbackRegistry};
pub use validators::{ValidatorFn, ValidatorRegistry, NamedValidator};
pub use validators::{
    assure_string_not_empty,
    assure_string_length,
    assure_string_not_length,
    assure_string_length_greater_than,
    assure_string_contains,
    assure_string_not_contains,
    assure_string_has_prefix,
    assure_string_no_prefix,
    assure_string_has_suffix,
    assure_string_no_suffix,
    assure_string_no_prefixes,
    assure_string_no_suffixes,
    assure_bool_true,
    assure_bool_false,
    assure_int_positive,
    assure_int_negative,
    assure_int_greater_than,
    assure_int_less_than,
    assure_int_in_range,
    assure_int64_positive,
    assure_int64_greater_than,
    assure_int64_less_than,
    assure_int64_in_range,
    assure_int128_positive,
    assure_int128_greater_than,
    assure_int128_less_than,
    assure_int128_in_range,
    assure_float64_not_nan,
    assure_float64_positive,
    assure_float64_greater_than,
    assure_float64_less_than,
    assure_float64_in_range,
    assure_float128_not_nan,
    assure_float128_positive,
    assure_float128_in_range,
    assure_duration_positive,
    assure_duration_greater_than,
    assure_duration_less_than,
    assure_duration_min,
    assure_duration_max,
    assure_list_not_empty,
    assure_list_min_length,
    assure_list_length,
    assure_list_not_length,
    assure_list_contains,
    assure_list_not_contains,
    assure_map_not_empty,
    assure_map_length,
    assure_map_not_length,
    assure_map_has_key,
    assure_map_has_keys,
    assure_map_value_matches,
};
pub use priority::{Source, ResolutionResult, resolve, resolve_all};
