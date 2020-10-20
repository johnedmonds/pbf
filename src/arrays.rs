use js_sys::{ArrayBuffer, Uint8Array};

pub fn make_typed_array(arr: &[u8]) -> Uint8Array {
    let out_arr = Uint8Array::new_with_length(arr.len() as u32);
    for i in 0..arr.len() {
        out_arr.set_index(i as u32, arr[i])
    }
    out_arr
}

pub fn array_buffer_to_vec(arr: ArrayBuffer) -> Vec<u8> {
    let arr = Uint8Array::new_with_byte_offset(&arr, 0);
    let mut out: Vec<u8> = Vec::with_capacity(arr.length() as usize);
    for i in 0..arr.length() {
        out.push(arr.get_index(i));
    }
    out
}
