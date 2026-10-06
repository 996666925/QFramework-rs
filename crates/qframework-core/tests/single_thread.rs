use std::cell::{Cell, RefCell};
use std::rc::Rc;

use qframework_core::prelude::*;

#[derive(Default, IModel)]
struct LocalModel<T: 'static> {
    arch: ArchRef,
    value: Rc<RefCell<T>>,
}

#[derive(Default, ISystem)]
struct LocalSystem {
    arch: ArchRef,
    calls: Cell<usize>,
}

#[derive(Default, IUtility)]
struct LocalUtility {
    text: Rc<RefCell<String>>,
}

struct LocalCommand(Rc<String>);
impl ICommand for LocalCommand {
    type Output = Rc<String>;
    fn execute(&self, ctx: &CommandContext) -> Self::Output {
        *ctx.get_model::<LocalModel<String>>().value.borrow_mut() = (*self.0).clone();
        self.0.clone()
    }
}

struct LocalQuery(Rc<()>);
impl IQuery for LocalQuery {
    type Result = Rc<RefCell<String>>;
    fn do_query(&self, ctx: &QueryContext) -> Self::Result {
        let _ = &self.0;
        ctx.get_model::<LocalModel<String>>().value.clone()
    }
}

#[test]
fn layers_commands_and_results_accept_single_thread_objects() {
    let architecture = ArchitectureBuilder::new()
        .model(LocalModel::<String>::default())
        .system(LocalSystem::default())
        .utility(LocalUtility::default())
        .build();
    let input = Rc::new("count".to_owned());
    let output = architecture.send_command(LocalCommand(input.clone()));
    assert!(Rc::ptr_eq(&input, &output));
    let value = architecture.send_query(LocalQuery(Rc::new(())));
    assert_eq!(&*value.borrow(), "count");
    architecture.get_system::<LocalSystem>().calls.set(1);
    architecture
        .get_utility::<LocalUtility>()
        .text
        .borrow_mut()
        .push_str("local");
    architecture.deinit();
}

#[test]
fn properties_and_collections_accept_rc_values_and_mutable_captures() {
    let property = BindableProperty::new(Rc::new(1));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let mut calls = 0;
    let unregister = property.register_with_init_value(move |value| {
        calls += 1;
        sink.borrow_mut().push((calls, **value));
    });
    property.set(Rc::new(2));
    assert_eq!(*seen.borrow(), vec![(1, 1), (2, 2)]);
    unregister.unregister();

    let list = BindableList::new();
    list.add(Rc::new(3));
    assert_eq!(**list.get(0).as_ref().unwrap(), 3);
    let dictionary = BindableDictionary::new();
    dictionary.insert(Rc::new("key"), Rc::new(4));
    assert_eq!(**dictionary.get(&Rc::new("key")).as_ref().unwrap(), 4);
}

#[test]
fn property_notifications_release_value_borrow_before_callbacks() {
    let property = BindableProperty::new(0);
    let captured = property.clone();
    let unregister = property.register(move |value| {
        assert_eq!(captured.get(), *value);
        captured.set_value_without_event(value + 1);
    });
    property.modify(|value| *value = 5);
    assert_eq!(property.get(), 6);
    unregister.unregister();
}

#[test]
fn initial_callback_failure_unregisters_and_releases_captures() {
    let property = BindableProperty::new(0);
    let capture = Rc::new(());
    let captured = capture.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        property.register_with_init_value(move |_| {
            let _ = &captured;
            panic!("initial callback failed");
        });
    }));
    assert!(result.is_err());
    assert_eq!(property.handler_count(), 0);
    assert_eq!(Rc::strong_count(&capture), 1);
}

#[test]
fn event_callback_can_unregister_itself_and_register_another() {
    let bus = Rc::new(TypeEventSystem::new());
    let slot = Rc::new(RefCell::new(None::<IUnRegister>));
    let calls = Rc::new(Cell::new(0));
    let weak_slot = Rc::downgrade(&slot);
    let weak_bus = Rc::downgrade(&bus);
    let sink = calls.clone();
    *slot.borrow_mut() = Some(bus.register(move |_: &i32| {
        sink.set(sink.get() + 1);
        weak_slot
            .upgrade()
            .unwrap()
            .borrow_mut()
            .take()
            .unwrap()
            .unregister();
        let other_sink = sink.clone();
        weak_bus
            .upgrade()
            .unwrap()
            .register(move |_: &i32| other_sink.set(other_sink.get() + 10));
    }));
    bus.send(1);
    assert_eq!(calls.get(), 1);
    bus.send(2);
    assert_eq!(calls.get(), 11);
    bus.clear();
    assert_eq!(Rc::strong_count(&calls), 1);
}

#[test]
fn recursive_callback_panics_without_poisoning_future_notifications() {
    let property = BindableProperty::new(0);
    let captured = property.clone();
    let unregister = property.register(move |value| {
        if *value == 1 {
            captured.set(2);
        }
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| property.set(1)));
    assert!(result.is_err());
    property.set(3);
    assert_eq!(property.get(), 3);
    unregister.unregister();
}
