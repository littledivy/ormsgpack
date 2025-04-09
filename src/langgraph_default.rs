use pyo3::ffi::*;
use serde::ser::SerializeSeq;
use serde_bytes::ByteBuf;

use crate::ext::PyExt;
use crate::raise_packb_exception;
use crate::serialize::serializer::MessagePackSerializer;
use crate::serialize::str::Str;
use crate::serialize::writer::BytesWriter;
use crate::typeref::STR_TYPE;

use serde::Serialize;
use serde::Serializer;

const EXT_CONSTRUCTOR_SINGLE_ARG: u32 = 0;
const EXT_PYDANTIC_V2: u32 = 4;

fn ext(tag: u32, data: *mut PyObject) -> *mut PyObject {
    let tag = unsafe { PyLong_FromUnsignedLong(tag as _) };
    PyExt::create(tag, data)
}

#[no_mangle]
pub unsafe extern "C" fn msgpack_default(
    _self: *mut PyObject,
    args: *const *mut PyObject,
    nargs: Py_ssize_t,
    kwnames: *mut PyObject,
) -> *mut PyObject {
    let nargs = PyVectorcall_NARGS(nargs as usize);
    if unlikely!(nargs == 0) {
        return raise_packb_exception("_msgpack_default() requires at least 1 argument");
    }

    unsafe fn callable_attr(obj: *mut PyObject, attr: *const i8) -> Option<*mut PyObject> {
        let callable = get_attr(obj, attr)?;
        if PyCallable_Check(callable) == 0 {
            Py_DECREF(callable);
            return None;
        }
        Some(callable)
    }

    unsafe fn get_attr(obj: *mut PyObject, attr: *const i8) -> Option<*mut PyObject> {
        let mut result = std::ptr::null_mut();
        let attr = PyObject_GetOptionalAttrString(obj, attr, &mut result);
        if attr == 1 {
            return Some(result);
        }
        None

    }

    let mut buf = BytesWriter::default();
    let mut ser = MessagePackSerializer::new(&mut buf);

    // pydantic v2
    if let Some(model_dump) = callable_attr(*args, c"model_dump".as_ptr()) {
        let model = PyObject_CallObject(model_dump, std::ptr::null_mut());
        Py_DECREF(model_dump);
        if model.is_null() {
            return raise_packb_exception("model_dump() returned None");
        }

        let class = get_attr(*args, c"__class__".as_ptr()).unwrap();
        let module = get_attr(class, c"__module__".as_ptr()).unwrap();
        let name = get_attr(class, c"__name__".as_ptr()).unwrap();

        {
            let mut seq = ser.serialize_seq(Some(3)).unwrap();
            seq.serialize_element(&Str::new(module));
            seq.serialize_element(&Str::new(name));
            seq.serialize_element(&Str::new(model_dump));
            seq.serialize_element("model_validate_json");
            seq.end();
        }

        Py_DECREF(model);

        // TODO: tag with Ext
        let buf = buf.finish();
        return ext(EXT_PYDANTIC_V2, buf.as_ptr());
    }

    // convert set to tuple
    if PySet_Check(*args) != 0 {
        // t = tuple(args)

        let class = get_attr(*args, c"__class__".as_ptr()).unwrap();
        let module = get_attr(class, c"__module__".as_ptr()).unwrap();
        let name = get_attr(class, c"__name__".as_ptr()).unwrap();

        {
            let mut seq = ser.serialize_seq(Some(2)).unwrap();
            seq.serialize_element(&Str::new(module));
            seq.serialize_element(&Str::new(name));
            seq.end();
        }

        let buf = buf.finish();
        let s = ext(EXT_CONSTRUCTOR_SINGLE_ARG, buf.as_ptr());

        return s;
    }

    todo!();
}
