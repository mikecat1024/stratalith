use crate::tree;
use std::cell::RefCell;

#[derive(Default)]
struct Buffers {
    input: Vec<u8>,
    output: Vec<u8>,
}

thread_local! { static BUFFERS: RefCell<Buffers> = RefCell::default(); }

/// The returned input buffer is writable until the next call to this function.
#[unsafe(no_mangle)]
pub extern "C" fn prepare_markdown_input(len: usize) -> *mut u8 {
    BUFFERS.with_borrow_mut(|buffers| {
        buffers.input.resize(len, 0);
        buffers.input.as_mut_ptr()
    })
}

/// Converts Markdown to a tree (0) or a tree to Markdown (1).
/// On failure the output contains a UTF-8 diagnostic. The output remains valid
/// until the next conversion.
#[unsafe(no_mangle)]
pub extern "C" fn convert_markdown(direction: u32) -> bool {
    convert(|input| match direction {
        0 => std::str::from_utf8(input)
            .map_err(|error| error.to_string())
            .and_then(|source| stratalith_markdown::parse(source).map_err(|error| error.to_string()))
            .map(|document| tree::encode(&document)),
        1 => tree::decode(input).and_then(|document| {
            stratalith_markdown::serialize(&document)
                .map(String::into_bytes)
                .map_err(|error| error.to_string())
        }),
        _ => Err("Unknown conversion".into()),
    })
}

pub(super) fn convert(operation: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>) -> bool {
    BUFFERS.with_borrow_mut(|buffers| {
        let result = operation(&buffers.input);
        match result {
            Ok(output) => {
                buffers.output = output;
                true
            }
            Err(message) => {
                buffers.output = message.into_bytes();
                false
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn markdown_output_pointer() -> *const u8 {
    BUFFERS.with_borrow(|buffers| buffers.output.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn markdown_output_length() -> usize {
    BUFFERS.with_borrow(|buffers| buffers.output.len())
}
