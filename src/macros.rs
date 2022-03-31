//! Macros used in the crate source.

macro_rules! debug_handleallocerror_precondition {
    ($condition:expr, $layout:ident) => {
        mirai_annotations::precondition!($condition);
        if cfg!(debug_assertions) {
            // check that `layout` is a valid layout
            if !($condition) {
                handle_alloc_error($layout);
            }
        }
    };
}

macro_rules! debug_handleallocerror_precondition_valid_layout {
    ($layout:ident) => {
        mirai_annotations::precondition!(
            Layout::from_size_align($layout.size(), $layout.align()).is_ok(),
            "invalid layout"
        );
        if cfg!(debug_assertions) {
            // check that `layout` is a valid layout
            if Layout::from_size_align($layout.size(), $layout.align()).is_err() {
                handle_alloc_error($layout);
            }
        }
    };
}

macro_rules! precondition_memory_range {
    ($ptr:expr, $len:expr) => {
        mirai_annotations::precondition!(!($ptr.is_null()), "null pointer is never valid");
        mirai_annotations::precondition!(
            ($ptr as usize).checked_add($len).is_some(),
            "memory range wraps the address space"
        );
    };
}

macro_rules! debug_precondition_logaligned {
    ($logalign:expr, $ptr:expr) => {
        mirai_annotations::debug_checked_precondition!(2_usize.checked_pow($logalign.into()).is_some(), "alignment must fit a usize");
        mirai_annotations::debug_checked_precondition_eq!(($ptr as usize) % 2_usize.pow($logalign.into()), 0, "pointer must be aligned");
    };
}

pub(crate) use {
    debug_handleallocerror_precondition, debug_handleallocerror_precondition_valid_layout,
    precondition_memory_range, debug_precondition_logaligned,
};
