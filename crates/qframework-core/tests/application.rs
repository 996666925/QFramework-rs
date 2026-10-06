use std::cell::Cell;
use std::rc::Rc;

use qframework_core::prelude::*;

#[derive(Default, IModel)]
struct CounterModel {
    arch: ArchRef,
    count: BindableProperty<i32>,
}

struct Increase;

impl ICommand for Increase {
    type Output = ();

    fn execute(&self, ctx: &CommandContext) {
        ctx.get_model::<CounterModel>().count.modify(|n| *n += 1);
    }
}

#[test]
fn controllers_share_application_without_injection_but_accept_explicit_overrides() {
    struct CounterApp;
    impl QApplication for CounterApp {
        fn build() -> ArchitectureBuilder {
            ArchitectureBuilder::new("Shared").model(CounterModel::default())
        }
    }

    #[derive(Default, IController)]
    #[controller(architecture = CounterApp)]
    struct Controller {
        #[arch]
        context: ArchRef,
    }

    let first = Controller::default();
    let second = Controller::default();
    first.send_command(Increase);
    assert_eq!(second.get_model::<CounterModel>().count.get(), 1);
    assert!(Rc::ptr_eq(&first.architecture(), &CounterApp::interface()));

    let isolated = CounterApp::build().build();
    let injected = isolated.attach_controller(Controller::default());
    injected.send_command(Increase);
    assert!(Rc::ptr_eq(&injected.architecture(), &isolated));
    assert_eq!(second.get_model::<CounterModel>().count.get(), 1);
    CounterApp::deinit_interface();
    isolated.deinit();
}

#[test]
fn repeated_access_initializes_once() {
    thread_local! { static INITS: Cell<usize> = const { Cell::new(0) }; }
    #[derive(Default, IModel)]
    #[model(init = |this: &Model| { INITS.set(INITS.get() + 1); this.value.set(42); })]
    struct Model {
        arch: ArchRef,
        value: BindableProperty<i32>,
    }
    struct App;
    impl QApplication for App {
        fn build() -> ArchitectureBuilder {
            ArchitectureBuilder::new("Once").model(Model::default())
        }
    }
    let first = App::interface();
    for _ in 0..8 {
        assert!(Rc::ptr_eq(&first, &App::interface()));
        assert_eq!(App::interface().get_model::<Model>().value.get(), 42);
    }
    assert_eq!(INITS.get(), 1);
    App::deinit_interface();
}

#[test]
fn application_types_and_fresh_instances_are_isolated() {
    struct App<T>(std::marker::PhantomData<T>);
    impl<T: 'static> QApplication for App<T> {
        fn build() -> ArchitectureBuilder {
            ArchitectureBuilder::new("Generic").model(CounterModel::default())
        }
    }

    #[derive(Default, IController)]
    #[controller(architecture = App::<T>)]
    struct Controller<T: 'static> {
        arch: ArchRef,
        marker: std::marker::PhantomData<T>,
    }

    Controller::<u32>::default().send_command(Increase);
    assert_eq!(
        App::<u32>::interface()
            .get_model::<CounterModel>()
            .count
            .get(),
        1
    );
    assert_eq!(
        App::<u64>::interface()
            .get_model::<CounterModel>()
            .count
            .get(),
        0
    );
    assert!(!Rc::ptr_eq(
        &App::<u32>::interface(),
        &App::<u64>::interface()
    ));
    let fresh = App::<u32>::build().build();
    assert_eq!(fresh.get_model::<CounterModel>().count.get(), 0);
    fresh.deinit();
    App::<u32>::deinit_interface();
    App::<u64>::deinit_interface();
}

#[test]
fn lifecycle_hooks_can_access_the_same_interface_and_reset_builds_a_new_instance() {
    thread_local! { static DEINITS: Cell<usize> = const { Cell::new(0) }; }

    #[derive(Default, IModel)]
    #[model(
        init = |this: &Model| assert!(Rc::ptr_eq(&this.architecture(), &App::interface())),
        deinit = |this: &Model| {
            assert!(Rc::ptr_eq(&this.architecture(), &App::interface()));
            DEINITS.replace(DEINITS.get() + 1);
        }
    )]
    struct Model {
        arch: ArchRef,
    }
    struct App;
    impl QApplication for App {
        fn build() -> ArchitectureBuilder {
            ArchitectureBuilder::new("Lifecycle").model(Model::default())
        }
    }

    let original = App::interface();
    App::deinit_interface();
    App::deinit_interface();
    assert!(!original.is_inited());
    assert_eq!(DEINITS.get(), 1);
    let next = App::interface();
    assert!(next.is_inited());
    assert!(!Rc::ptr_eq(&original, &next));
    App::deinit_interface();
}

#[test]
fn failed_initialization_can_be_retried() {
    thread_local! { static ATTEMPTS: Cell<usize> = const { Cell::new(0) }; }
    struct App;
    impl QApplication for App {
        fn build() -> ArchitectureBuilder {
            ArchitectureBuilder::new("Retry").patch(|_| {
                if ATTEMPTS.replace(ATTEMPTS.get() + 1) == 0 {
                    panic!("initialization failed");
                }
            })
        }
    }
    assert!(std::panic::catch_unwind(App::interface).is_err());
    assert!(App::interface().is_inited());
    assert_eq!(ATTEMPTS.get(), 2);
    App::deinit_interface();
}

#[test]
fn recursive_build_panics_without_poisoning_the_slot() {
    thread_local! { static ATTEMPTS: Cell<usize> = const { Cell::new(0) }; }
    struct App;
    impl QApplication for App {
        fn build() -> ArchitectureBuilder {
            if ATTEMPTS.replace(ATTEMPTS.get() + 1) == 0 {
                App::interface();
            }
            ArchitectureBuilder::new("Recursive")
        }
    }
    assert!(std::panic::catch_unwind(App::interface).is_err());
    assert!(App::interface().is_inited());
    App::deinit_interface();
}

#[test]
fn expired_injected_controller_does_not_fall_back_to_shared_application() {
    struct App;
    impl QApplication for App {
        fn build() -> ArchitectureBuilder {
            panic!("expired injected controller must not create the shared application");
        }
    }
    #[derive(Default, IController)]
    #[controller(architecture = App)]
    struct Controller {
        arch: ArchRef,
    }
    let controller = Controller::default();
    let isolated = ArchitectureBuilder::new("Temporary").build();
    controller.arch_ref().set(&isolated);
    drop(isolated);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| controller.architecture()))
            .is_err()
    );
}
