/*
 * Various types to support iteration.
 */

use std::cell::Cell;

use crate::pyobject::{PyContext, PyObjectRef, PyRef, PyResult, PyValue};
use crate::vm::VirtualMachine;

use super::objbytearray::PyByteArray;
use super::objbytes::PyBytes;
use super::objrange::PyRange;
use super::objsequence;
use super::objtype;
use super::objtype::PyClassRef;

/*
 * This helper function is called at multiple places. First, it is called
 * in the vm when a for loop is entered. Next, it is used when the builtin
 * function 'iter' is called.
 */
pub fn get_iter(vm: &VirtualMachine, iter_target: &PyObjectRef) -> PyResult {
    vm.call_method(iter_target, "__iter__", vec![])
    // let type_str = objstr::get_value(&vm.to_str(iter_target.class()).unwrap());
    // let type_error = vm.new_type_error(format!("Cannot iterate over {}", type_str));
    // return Err(type_error);

    // TODO: special case when iter_target only has __getitem__
    // see: https://docs.python.org/3/library/functions.html#iter
    // also https://docs.python.org/3.8/reference/datamodel.html#special-method-names
}

pub fn call_next(vm: &VirtualMachine, iter_obj: &PyObjectRef) -> PyResult {
    vm.call_method(iter_obj, "__next__", vec![])
}

/*
 * Helper function to retrieve the next object (or none) from an iterator.
 */
pub fn get_next_object(
    vm: &VirtualMachine,
    iter_obj: &PyObjectRef,
) -> PyResult<Option<PyObjectRef>> {
    let next_obj: PyResult = call_next(vm, iter_obj);

    match next_obj {
        Ok(value) => Ok(Some(value)),
        Err(next_error) => {
            // Check if we have stopiteration, or something else:
            if objtype::isinstance(&next_error, &vm.ctx.exceptions.stop_iteration) {
                Ok(None)
            } else {
                Err(next_error)
            }
        }
    }
}

/* Retrieve all elements from an iterator */
pub fn get_all(vm: &VirtualMachine, iter_obj: &PyObjectRef) -> PyResult<Vec<PyObjectRef>> {
    let mut elements = vec![];
    loop {
        let element = get_next_object(vm, iter_obj)?;
        match element {
            Some(v) => elements.push(v),
            None => break,
        }
    }
    Ok(elements)
}

pub fn new_stop_iteration(vm: &VirtualMachine) -> PyObjectRef {
    let stop_iteration_type = vm.ctx.exceptions.stop_iteration.clone();
    vm.new_exception(stop_iteration_type, "End of iterator".to_string())
}

// TODO: This is a workaround and shouldn't exist.
//       Each iterable type should have its own distinct iterator type.
// (however, this boilerplate can be reused for "generic iterator" for types with only __getiter__)
#[derive(Debug)]
pub struct PyIteratorValue {
    pub position: Cell<usize>,
    pub iterated_obj: PyObjectRef,
}

impl PyValue for PyIteratorValue {
    fn class(vm: &VirtualMachine) -> PyClassRef {
        vm.ctx.iter_type()
    }
}

type PyIteratorValueRef = PyRef<PyIteratorValue>;

impl PyIteratorValueRef {
    fn next(self, vm: &VirtualMachine) -> PyResult {
        let position = &self.position;
        let iterated_obj_ref = &self.iterated_obj;
        if let Some(range) = iterated_obj_ref.payload::<PyRange>() {
            if let Some(int) = range.get(position.get()) {
                position.set(position.get() + 1);
                Ok(vm.ctx.new_int(int))
            } else {
                Err(new_stop_iteration(vm))
            }
        } else if let Some(bytes) = iterated_obj_ref.payload::<PyBytes>() {
            if position.get() < bytes.len() {
                let obj_ref = vm.ctx.new_int(bytes[position.get()]);
                position.set(position.get() + 1);
                Ok(obj_ref)
            } else {
                Err(new_stop_iteration(vm))
            }
        } else if let Some(bytes) = iterated_obj_ref.payload::<PyByteArray>() {
            if position.get() < bytes.value.borrow().len() {
                let obj_ref = vm.ctx.new_int(bytes.value.borrow()[position.get()]);
                position.set(position.get() + 1);
                Ok(obj_ref)
            } else {
                Err(new_stop_iteration(vm))
            }
        } else {
            let elements = objsequence::get_elements(iterated_obj_ref);
            if position.get() < elements.len() {
                let obj_ref = elements[position.get()].clone();
                position.set(position.get() + 1);
                Ok(obj_ref)
            } else {
                Err(new_stop_iteration(vm))
            }
        }
    }

    fn iter(self, _vm: &VirtualMachine) -> Self {
        self
    }
}

pub fn init(context: &PyContext) {
    let iter_type = &context.iter_type;

    let iter_doc = "iter(iterable) -> iterator\n\
                    iter(callable, sentinel) -> iterator\n\n\
                    Get an iterator from an object.  In the first form, the argument must\n\
                    supply its own iterator, or be a sequence.\n\
                    In the second form, the callable is called until it returns the sentinel.";

    extend_class!(context, iter_type, {
        "__next__" => context.new_rustfunc(PyIteratorValueRef::next),
        "__iter__" => context.new_rustfunc(PyIteratorValueRef::iter),
        "__doc__" => context.new_str(iter_doc.to_string()),
    });
}
