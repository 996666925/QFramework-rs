//! `BindableProperty` / `BindableList` / `BindableDictionary` 的行为测试。

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use qframework_core::prelude::*;

#[test]
fn property_modify_notifies_once_with_written_value() {
    let property = BindableProperty::new(0);

    let calls = Rc::new(Cell::new(0));
    let last = Rc::new(Cell::new(-1));
    let call_sink = Rc::clone(&calls);
    let value_sink = Rc::clone(&last);

    let unregister = property.register(move |value| {
        call_sink.set(call_sink.get() + 1);
        value_sink.set(*value);
    });

    property.modify(|count| *count += 5);
    assert_eq!(property.get(), 5);
    assert_eq!(calls.get(), 1);
    assert_eq!(last.get(), 5);

    property.set(9);
    assert_eq!(calls.get(), 2);
    assert_eq!(last.get(), 9);

    unregister.unregister();
}

#[test]
fn register_with_init_value_delivers_current_value_first() {
    let property = BindableProperty::new(7);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&seen);

    let unregister = property.register_with_init_value(move |value| {
        sink.borrow_mut().push(*value);
    });

    property.set(8);
    unregister.unregister();
    property.set(9);

    assert_eq!(*seen.borrow_mut(), vec![7, 8]);
}

#[test]
fn with_value_reads_without_cloning() {
    let property = BindableProperty::new(vec![1, 2, 3]);
    assert_eq!(property.with_value(|values| values.len()), 3);
    assert_eq!(property.with_value(|values| values[1]), 2);
}

#[test]
fn bindable_list_emits_add_remove_and_count_events() {
    let list = BindableList::new();

    let adds = Rc::new(Cell::new(0));
    let counts = Rc::new(Cell::new(-1));
    let add_sink = Rc::clone(&adds);
    let count_sink = Rc::clone(&counts);

    let _on_add = list.on_add(move |event| {
        assert_eq!(event.index, 0);
        assert_eq!(event.value, 10);
        add_sink.set(add_sink.get() + 1);
    });
    let _on_count = list.on_count_changed(move |event| {
        count_sink.set(event.count as i32);
    });

    assert_eq!(list.add(10), 0);
    assert_eq!(list.len(), 1);
    assert_eq!(adds.get(), 1);
    assert_eq!(counts.get(), 1);

    assert_eq!(list.remove_at(0), Some(10));
    assert!(list.is_empty());
    assert_eq!(counts.get(), 0);
}

#[test]
fn bindable_list_with_items_avoids_snapshot_clone() {
    let list = BindableList::from_vec(vec![1, 2, 3]);

    assert_eq!(list.with_items(|items| items.iter().sum::<i32>()), 6);
    assert_eq!(list.snapshot(), vec![1, 2, 3]);

    list.insert(0, 0);
    assert_eq!(list.snapshot(), vec![0, 1, 2, 3]);

    list.clear();
    assert!(list.is_empty());
}

#[test]
fn bindable_dictionary_emits_add_replace_remove() {
    let dictionary = BindableDictionary::new();

    let adds = Rc::new(Cell::new(0));
    let replaces = Rc::new(Cell::new(0));
    let add_sink = Rc::clone(&adds);
    let replace_sink = Rc::clone(&replaces);

    let _on_add = dictionary.on_add(move |_| {
        add_sink.set(add_sink.get() + 1);
    });
    let _on_replace = dictionary.on_replace(move |event| {
        assert_eq!(event.previous, 1);
        assert_eq!(event.current, 2);
        replace_sink.set(replace_sink.get() + 1);
    });

    dictionary.insert("hp", 1);
    assert_eq!(adds.get(), 1);
    assert_eq!(dictionary.get(&"hp"), Some(1));

    dictionary.insert("hp", 2);
    assert_eq!(adds.get(), 1);
    assert_eq!(replaces.get(), 1);
    assert_eq!(dictionary.get(&"hp"), Some(2));

    assert!(dictionary.contains_key(&"hp"));
    assert_eq!(dictionary.with_entries(HashMap::len), 1);

    assert_eq!(dictionary.remove(&"hp"), Some(2));
    assert!(dictionary.is_empty());
    assert_eq!(dictionary.get(&"hp"), None);
}
