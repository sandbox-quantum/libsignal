//
// Copyright 2020 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use libc::{c_uchar, size_t};
use libsignal_protocol_rust::*;

mod error;
pub use error::*;

pub fn run_ffi_safe<F: FnOnce() -> Result<(), SignalFfiError> + std::panic::UnwindSafe>(
    f: F,
) -> *mut SignalFfiError {
    let result = match std::panic::catch_unwind(f) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(r) => Err(SignalFfiError::UnexpectedPanic(r)),
    };

    match result {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => Box::into_raw(Box::new(e)),
    }
}

pub unsafe fn box_object<T>(
    p: *mut *mut T,
    obj: Result<T, SignalProtocolError>,
) -> Result<(), SignalFfiError> {
    if p.is_null() {
        return Err(SignalFfiError::NullPointer);
    }
    match obj {
        Ok(o) => {
            *p = Box::into_raw(Box::new(o));
            Ok(())
        }
        Err(e) => {
            *p = std::ptr::null_mut();
            Err(SignalFfiError::Signal(e))
        }
    }
}

pub unsafe fn native_handle_cast<T>(handle: *const T) -> Result<&'static T, SignalFfiError> {
    if handle.is_null() {
        return Err(SignalFfiError::NullPointer);
    }

    Ok(&*(handle))
}

pub unsafe fn write_bytearray_to<T: Into<Box<[u8]>>>(
    out: *mut *const c_uchar,
    out_len: *mut size_t,
    value: Result<T, SignalProtocolError>,
) -> Result<(), SignalFfiError> {
    if out.is_null() || out_len.is_null() {
        return Err(SignalFfiError::NullPointer);
    }

    match value {
        Ok(value) => {
            let value: Box<[u8]> = value.into();

            *out_len = value.len();
            let mem = Box::into_raw(value);
            *out = (*mem).as_ptr();

            Ok(())
        }
        Err(e) => Err(SignalFfiError::Signal(e)),
    }
}

macro_rules! ffi_bridge_destroy {
    ( $typ:ty as None ) => {};
    ( $typ:ty as $ffi_name:ident ) => {
        paste! {
            #[cfg(feature = "ffi")]
            #[no_mangle]
            pub unsafe extern "C" fn [<signal_ $ffi_name _destroy>](
                p: *mut $typ
            ) -> *mut ffi::SignalFfiError {
                ffi::run_ffi_safe(|| {
                    if !p.is_null() {
                        Box::from_raw(p);
                    }
                    Ok(())
                })
            }
        }
    };
    ( $typ:ty ) => {
        paste! {
            ffi_bridge_destroy!($typ as [<$typ:snake>]);
        }
    };
}

macro_rules! ffi_bridge_deserialize {
    ( $typ:ident::$fn:path as None ) => {};
    ( $typ:ident::$fn:path as $ffi_name:ident ) => {
        paste! {
            #[cfg(feature = "ffi")]
            #[no_mangle]
            pub unsafe extern "C" fn [<signal_ $ffi_name _deserialize>](
                p: *mut *mut $typ,
                data: *const libc::c_uchar,
                data_len: libc::size_t,
            ) -> *mut ffi::SignalFfiError {
                ffi::run_ffi_safe(|| {
                    if data.is_null() {
                        return Err(ffi::SignalFfiError::NullPointer);
                    }
                    let data = std::slice::from_raw_parts(data, data_len);
                    ffi::box_object(p, $typ::$fn(data))
                })
            }
        }
    };
    ( $typ:ident::$fn:path ) => {
        paste! {
            ffi_bridge_deserialize!($typ::$fn as [<$typ:snake>]);
        }
    };
}

macro_rules! ffi_bridge_get_bytearray {
    ( $name:ident($typ:ty) as None => $body:expr ) => {};
    ( $name:ident($typ:ty) as $ffi_name:ident => $body:expr ) => {
        paste! {
            #[no_mangle]
            pub unsafe extern "C" fn [<signal_ $ffi_name>](
                obj: *const $typ,
                out: *mut *const libc::c_uchar,
                out_len: *mut libc::size_t,
            ) -> *mut ffi::SignalFfiError {
                ffi::run_ffi_safe(|| {
                    let obj = ffi::native_handle_cast::<$typ>(obj)?;
                    ffi::write_bytearray_to(out, out_len, $body(obj))
                })
            }
        }
    };
    ( $name:ident($typ:ty) => $body:expr ) => {
        paste! {
            ffi_bridge_get_bytearray!($name($typ) as [<$typ:snake _ $name>] => $body);
        }
    };
}

// Currently unneeded.
macro_rules! ffi_bridge_get_optional_bytearray {
    ( $name:ident($typ:ty) as None => $body:expr ) => {};
}
