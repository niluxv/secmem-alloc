//! Macros used in the crate source.

macro_rules! debug_handleallocerror_precondition {
    ($condition:expr, $layout:ident) => {
        mirai_annotations::precondition!($condition);
        if cfg!(debug_assertions) {
            // check that `layout` is a valid layout
            if !($condition) {
                alloc::alloc::handle_alloc_error($layout);
            }
        }
    };
}

macro_rules! debug_handleallocerror_precondition_valid_layout {
    ($layout:ident) => {
        mirai_annotations::precondition!(
            core::alloc::Layout::from_size_align($layout.size(), $layout.align()).is_ok(),
            "invalid layout"
        );
        if cfg!(debug_assertions) {
            // check that `layout` is a valid layout
            if core::alloc::Layout::from_size_align($layout.size(), $layout.align()).is_err() {
                alloc::alloc::handle_alloc_error($layout);
            }
        }
    };
}

macro_rules! precondition_memory_range {
    ($ptr:expr, $len:expr) => {
        mirai_annotations::precondition!(!($ptr.is_null()), "null pointer is never valid");
        mirai_annotations::precondition!(
            $ptr.addr().checked_add($len).is_some(),
            "memory range wraps the address space"
        );
    };
}

pub(crate) use {
    debug_handleallocerror_precondition, debug_handleallocerror_precondition_valid_layout,
    precondition_memory_range,
};
