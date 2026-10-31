use crate::native_appkit::*;
use crate::{
    AppleActivity, AppleDispatcher, BoolExt, MacDisplay, MacKeyboardLayout, MacKeyboardMapper,
    MacWindow, events::key_to_native, ns_string, pasteboard::Pasteboard, renderer,
    set_active_window_cursor_style,
};
use anyhow::{Context as _, anyhow};
use block2::RcBlock;
use objc2::{class, msg_send, sel};
use objc2_app_kit::*;
use objc2_foundation::*;

use core_foundation::{base::CFRelease, runloop::CFRunLoopRun, string::CFStringRef};
use ctor::ctor;
use dispatch2::DispatchQueue;
use futures::channel::oneshot;
use gpui::{
    Action, ActivationPolicy, ActivityGuard, AnyWindowHandle, BackgroundExecutor, ClipboardItem,
    CursorStyle, ForegroundExecutor, GraphicalEnvironment, KeyContext, Keymap, Menu, MenuItem,
    OsMenu, OwnedMenu, PathPromptOptions, Platform, PlatformDisplay, PlatformKeyboardLayout,
    PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow, Result, SystemMenuType, Task,
    ThermalState, WindowAppearance, WindowKind, WindowParams, WindowingRequest,
    popup::PopupNotSupportedError,
};
use gpui_apple::{
    keychain,
    thermal::{self, ThermalObserver},
};
use gpui_util::{ResultExt, new_std_command};
use itertools::Itertools;

use objc2::{MainThreadMarker, runtime::AnyObject};
use objc2_app_kit::{
    NSModalResponse, NSModalResponseOK, NSOpenPanel, NSSavePanel, NSWorkspace,
    NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification,
};
use objc2_foundation::NSActivityOptions;
use parking_lot::Mutex;
use ptr::null_mut;
use semver::Version;
use std::{
    cell::Cell,
    ffi::{CStr, OsStr, c_void},
    os::{raw::c_char, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
    ptr,
    rc::Rc,
    slice, str,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

#[allow(non_upper_case_globals)]
const NSUTF8StringEncoding: NSUInteger = 4;

const MAC_PLATFORM_IVAR: &str = "platform";
static mut APP_CLASS: *const Class = ptr::null();
static mut APP_DELEGATE_CLASS: *const Class = ptr::null();

#[ctor(unsafe)]
unsafe fn build_classes() {
    unsafe {
        APP_CLASS = {
            let mut decl = ClassBuilder::new(c"GPUIApplication", class!(NSApplication)).unwrap();
            decl.add_ivar::<*mut c_void>(c"platform");
            decl.register()
        }
    };
    unsafe {
        APP_DELEGATE_CLASS = {
            let mut decl =
                ClassBuilder::new(c"GPUIApplicationDelegate", class!(NSResponder)).unwrap();
            decl.add_ivar::<*mut c_void>(c"platform");
            decl.add_method(
                sel!(applicationWillFinishLaunching:),
                will_finish_launching as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationDidFinishLaunching:),
                did_finish_launching as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationShouldHandleReopen:hasVisibleWindows:),
                should_handle_reopen as extern "C-unwind" fn(*mut Object, Sel, id, BOOL) -> BOOL,
            );
            decl.add_method(
                sel!(applicationWillTerminate:),
                will_terminate as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(handleGPUIMenuItem:),
                handle_menu_item as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            // Add menu item handlers so that OS save panels have the correct key commands
            decl.add_method(
                sel!(cut:),
                handle_menu_item as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(copy:),
                handle_menu_item as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(paste:),
                handle_menu_item as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(selectAll:),
                handle_menu_item as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(undo:),
                handle_menu_item as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(redo:),
                handle_menu_item as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(validateMenuItem:),
                validate_menu_item as extern "C-unwind" fn(*mut Object, Sel, id) -> BOOL,
            );
            decl.add_method(
                sel!(menuWillOpen:),
                menu_will_open as extern "C-unwind" fn(*mut Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationDockMenu:),
                handle_dock_menu as extern "C-unwind" fn(*mut Object, Sel, id) -> id,
            );
            decl.add_method(
                sel!(application:openURLs:),
                open_urls as extern "C-unwind" fn(*mut Object, Sel, id, id),
            );

            decl.add_method(
                sel!(onKeyboardLayoutChange:),
                on_keyboard_layout_change as extern "C-unwind" fn(*mut Object, Sel, id),
            );

            decl.add_method(
                sel!(onScreenParametersChange:),
                on_screen_parameters_change as extern "C-unwind" fn(*mut Object, Sel, id),
            );

            decl.add_method(
                sel!(onSystemSleep:),
                on_system_sleep as extern "C-unwind" fn(*mut Object, Sel, id),
            );

            decl.add_method(
                sel!(onSystemWake:),
                on_system_wake as extern "C-unwind" fn(*mut Object, Sel, id),
            );

            decl.register()
        }
    }
}

pub struct MacPlatform(Mutex<MacPlatformState>, MainThreadMarker);

pub(crate) struct MacPlatformState {
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    renderer_context: renderer::Context,
    headless: bool,
    activation_policy: ActivationPolicy,
    application_created: bool,
    general_pasteboard: Pasteboard,
    find_pasteboard: Pasteboard,
    reopen: Option<Box<dyn FnMut()>>,
    on_keyboard_layout_change: Option<Box<dyn FnMut()>>,
    thermal_observer: Option<ThermalObserver>,
    on_displays_changed: Option<Box<dyn FnMut()>>,
    on_system_sleep: Option<Box<dyn FnMut()>>,
    on_system_wake: Option<Box<dyn FnMut()>>,
    system_power_observers_registered: bool,
    quit: Option<Box<dyn FnMut() -> bool>>,
    menu_command: Option<Box<dyn FnMut(&dyn Action)>>,
    validate_menu_command: Option<Box<dyn FnMut(&dyn Action) -> bool>>,
    will_open_menu: Option<Box<dyn FnMut()>>,
    menu_actions: Vec<Box<dyn Action>>,
    open_urls: Option<Box<dyn FnMut(Vec<String>)>>,
    finish_launching: Option<Box<dyn FnOnce()>>,
    dock_menu: Option<id>,
    menus: Option<Vec<OwnedMenu>>,
    keyboard_mapper: Rc<MacKeyboardMapper>,
    /// Mirrors `[NSCursor setHiddenUntilMouseMoves:]` state, which AppKit doesn't expose.
    cursor_visible: Arc<AtomicBool>,
    system_notifications: crate::system_notifications::SystemNotificationState,
}

/// The activation policy that stands for a windowing mode: an app is headless while it has no
/// Dock icon or menu bar.
fn activation_policy_for(request: &WindowingRequest) -> ActivationPolicy {
    match request {
        WindowingRequest::Headless => ActivationPolicy::Accessory,
        WindowingRequest::Windowed(_) => ActivationPolicy::Regular,
    }
}

fn native_activation_policy(policy: ActivationPolicy) -> NSApplicationActivationPolicy {
    match policy {
        ActivationPolicy::Regular => NSApplicationActivationPolicy::Regular,
        ActivationPolicy::Accessory => NSApplicationActivationPolicy::Accessory,
    }
}

impl MacPlatform {
    pub fn new(headless: bool) -> Self {
        let marker = MainThreadMarker::new().expect("Mac platform not created on main thread");
        let dispatcher = Arc::new(AppleDispatcher::new());

        #[cfg(feature = "font-kit")]
        let text_system = Arc::new(gpui_apple::AppleTextSystem::new());

        #[cfg(not(feature = "font-kit"))]
        let text_system = {
            if !headless {
                log::warn!(
                    "gpui_macos was compiled without the `font-kit` feature, so no text will be rendered."
                );
            }
            Arc::new(gpui::NoopTextSystem::new())
        };

        let keyboard_layout = MacKeyboardLayout::new();
        let keyboard_mapper = Rc::new(MacKeyboardMapper::new(keyboard_layout.id()));

        let state = Mutex::new(MacPlatformState {
            headless,
            activation_policy: ActivationPolicy::Regular,
            application_created: false,
            text_system,
            background_executor: BackgroundExecutor::new(dispatcher.clone()),
            foreground_executor: ForegroundExecutor::new(dispatcher),
            renderer_context: renderer::Context::default(),
            general_pasteboard: Pasteboard::general(),
            find_pasteboard: Pasteboard::find(),
            reopen: None,
            quit: None,
            menu_command: None,
            validate_menu_command: None,
            will_open_menu: None,
            menu_actions: Default::default(),
            open_urls: None,
            finish_launching: None,
            dock_menu: None,
            on_keyboard_layout_change: None,
            thermal_observer: None,
            on_displays_changed: None,
            on_system_sleep: None,
            on_system_wake: None,
            system_power_observers_registered: false,
            menus: None,
            keyboard_mapper,
            cursor_visible: Arc::new(AtomicBool::new(true)),
            system_notifications: crate::system_notifications::SystemNotificationState::new(),
        });
        Self(state, marker)
    }

    unsafe fn create_menu_bar(
        &self,
        menus: &Vec<Menu>,
        delegate: id,
        actions: &mut Vec<Box<dyn Action>>,
        keymap: &Keymap,
    ) -> id {
        unsafe {
            let application_menu = {
                let value: id = msg_send![
                    ({
                        let value: id = msg_send![class!(NSMenu), new];
                        value
                    }),
                    autorelease
                ];
                value
            };
            ({
                let value: () = msg_send![application_menu, setDelegate: delegate];
                value
            });

            for menu_config in menus {
                let menu = {
                    let value: id = msg_send![
                        ({
                            let value: id = msg_send![class!(NSMenu), new];
                            value
                        }),
                        autorelease
                    ];
                    value
                };
                let menu_title = ns_string(&menu_config.name);
                ({
                    let value: () = msg_send![menu, setTitle: menu_title];
                    value
                });
                ({
                    let value: () = msg_send![menu, setDelegate: delegate];
                    value
                });

                for item_config in &menu_config.items {
                    ({
                        let value: () = msg_send![menu, addItem: Self::create_menu_item(
                            item_config,
                            delegate,
                            actions,
                            keymap,
                        )];
                        value
                    });
                }

                let menu_item = {
                    let value: id = msg_send![
                        ({
                            let value: id = msg_send![class!(NSMenuItem), new];
                            value
                        }),
                        autorelease
                    ];
                    value
                };
                ({
                    let value: () = msg_send![menu_item, setTitle: menu_title];
                    value
                });
                ({
                    let value: () = msg_send![menu_item, setSubmenu: menu];
                    value
                });
                ({
                    let value: () = msg_send![application_menu, addItem: menu_item];
                    value
                });

                if menu_config.name == "Window" {
                    let app: id = msg_send![APP_CLASS, sharedApplication];
                    ({
                        let value: () = msg_send![app, setWindowsMenu: menu];
                        value
                    });
                }
            }

            application_menu
        }
    }

    unsafe fn create_dock_menu(
        &self,
        menu_items: Vec<MenuItem>,
        delegate: id,
        actions: &mut Vec<Box<dyn Action>>,
        keymap: &Keymap,
    ) -> id {
        {
            let dock_menu = unsafe {
                let value: id = msg_send![class!(NSMenu), new];
                value
            };
            (unsafe {
                let value: () = msg_send![dock_menu, setDelegate: delegate];
                value
            });
            for item_config in menu_items {
                (unsafe {
                    let value: () = msg_send![dock_menu, addItem: Self::create_menu_item(
                        &item_config,
                        delegate,
                        actions,
                        keymap,
                    )];
                    value
                });
            }

            dock_menu
        }
    }

    unsafe fn create_menu_item(
        item: &MenuItem,
        delegate: id,
        actions: &mut Vec<Box<dyn Action>>,
        keymap: &Keymap,
    ) -> id {
        static DEFAULT_CONTEXT: OnceLock<Vec<KeyContext>> = OnceLock::new();

        unsafe {
            match item {
                MenuItem::Separator => {
                    let value: id = msg_send![class!(NSMenuItem), separatorItem];
                    value
                }
                MenuItem::Action {
                    name,
                    action,
                    os_action,
                    checked,
                    disabled,
                } => {
                    // Note that this is intentionally using earlier bindings, whereas typically
                    // later ones take display precedence. See the discussion on
                    // https://github.com/zed-industries/zed/issues/23621
                    let keystrokes = keymap
                        .bindings_for_action(action.as_ref())
                        .find_or_first(|binding| {
                            binding.predicate().is_none_or(|predicate| {
                                predicate.eval(DEFAULT_CONTEXT.get_or_init(|| {
                                    let mut workspace_context = KeyContext::new_with_defaults();
                                    workspace_context.add("Workspace");
                                    let mut pane_context = KeyContext::new_with_defaults();
                                    pane_context.add("Pane");
                                    let mut editor_context = KeyContext::new_with_defaults();
                                    editor_context.add("Editor");

                                    pane_context.extend(&editor_context);
                                    workspace_context.extend(&pane_context);
                                    vec![workspace_context]
                                }))
                            })
                        })
                        .map(|binding| binding.keystrokes());

                    let selector = match os_action {
                        Some(gpui::OsAction::Cut) => register_selector("cut:"),
                        Some(gpui::OsAction::Copy) => register_selector("copy:"),
                        Some(gpui::OsAction::Paste) => register_selector("paste:"),
                        Some(gpui::OsAction::SelectAll) => register_selector("selectAll:"),
                        // "undo:" and "redo:" are always disabled in our case, as
                        // we don't have a NSTextView/NSTextField to enable them on.
                        Some(gpui::OsAction::Undo) => register_selector("handleGPUIMenuItem:"),
                        Some(gpui::OsAction::Redo) => register_selector("handleGPUIMenuItem:"),
                        None => register_selector("handleGPUIMenuItem:"),
                    };

                    let item;
                    if let Some(keystrokes) = keystrokes {
                        if keystrokes.len() == 1 {
                            let keystroke = &keystrokes[0];
                            let mut mask = NSEventModifierFlags::empty();
                            for (modifier, flag) in &[
                                (
                                    keystroke.modifiers().platform,
                                    NSEventModifierFlags::Command,
                                ),
                                (keystroke.modifiers().control, NSEventModifierFlags::Control),
                                (keystroke.modifiers().alt, NSEventModifierFlags::Option),
                                (keystroke.modifiers().shift, NSEventModifierFlags::Shift),
                            ] {
                                if *modifier {
                                    mask |= *flag;
                                }
                            }

                            item = {
                                let value: id = msg_send![
                                    ({
                                        let value: id = msg_send![ ( { let value: id = msg_send![class!(NSMenuItem), alloc]; value })
                                , initWithTitle: ns_string(name) action: selector keyEquivalent: ns_string(key_to_native(keystroke.key()).as_ref())];
                                        value
                                    }),
                                    autorelease
                                ];
                                value
                            };
                            if Self::os_version() >= Version::new(12, 0, 0) {
                                let _: () = msg_send![item, setAllowsAutomaticKeyEquivalentLocalization: NO];
                            }
                            ({
                                let value: () = msg_send![item, setKeyEquivalentModifierMask: mask];
                                value
                            });
                        } else {
                            item = {
                                let value: id = msg_send![
                                    ({
                                        let value: id = msg_send![ ( { let value: id = msg_send![class!(NSMenuItem), alloc]; value })
                                , initWithTitle: ns_string(name) action: selector keyEquivalent: ns_string("")];
                                        value
                                    }),
                                    autorelease
                                ];
                                value
                            };
                        }
                    } else {
                        item = {
                            let value: id = msg_send![
                                ({
                                    let value: id = msg_send![ ( { let value: id = msg_send![class!(NSMenuItem), alloc]; value })
                            , initWithTitle: ns_string(name) action: selector keyEquivalent: ns_string("")];
                                    value
                                }),
                                autorelease
                            ];
                            value
                        };
                    }

                    if *checked {
                        ({
                            let value: () = msg_send![item, setState: NSVisualEffectState::Active];
                            value
                        });
                    }
                    ({
                        let value: () =
                            msg_send![item, setEnabled: if *disabled { NO } else { YES }];
                        value
                    });

                    let tag = actions.len() as NSInteger;
                    let _: () = msg_send![item, setTag: tag];
                    actions.push(action.boxed_clone());
                    item
                }
                MenuItem::Submenu(Menu {
                    name,
                    items,
                    disabled,
                }) => {
                    let item = {
                        let value: id = msg_send![
                            ({
                                let value: id = msg_send![class!(NSMenuItem), new];
                                value
                            }),
                            autorelease
                        ];
                        value
                    };
                    let submenu = {
                        let value: id = msg_send![
                            ({
                                let value: id = msg_send![class!(NSMenu), new];
                                value
                            }),
                            autorelease
                        ];
                        value
                    };
                    ({
                        let value: () = msg_send![submenu, setDelegate: delegate];
                        value
                    });
                    for item in items {
                        ({
                            let value: () = msg_send![submenu, addItem: Self::create_menu_item(item, delegate, actions, keymap)];
                            value
                        });
                    }
                    ({
                        let value: () = msg_send![item, setSubmenu: submenu];
                        value
                    });
                    ({
                        let value: () =
                            msg_send![item, setEnabled: if *disabled { NO } else { YES }];
                        value
                    });
                    ({
                        let value: () = msg_send![item, setTitle: ns_string(name)];
                        value
                    });
                    item
                }
                MenuItem::SystemMenu(OsMenu { name, menu_type }) => {
                    let item = {
                        let value: id = msg_send![
                            ({
                                let value: id = msg_send![class!(NSMenuItem), new];
                                value
                            }),
                            autorelease
                        ];
                        value
                    };
                    let submenu = {
                        let value: id = msg_send![
                            ({
                                let value: id = msg_send![class!(NSMenu), new];
                                value
                            }),
                            autorelease
                        ];
                        value
                    };
                    ({
                        let value: () = msg_send![submenu, setDelegate: delegate];
                        value
                    });
                    ({
                        let value: () = msg_send![item, setSubmenu: submenu];
                        value
                    });
                    ({
                        let value: () = msg_send![item, setTitle: ns_string(name)];
                        value
                    });

                    match menu_type {
                        SystemMenuType::Services => {
                            let app: id = msg_send![APP_CLASS, sharedApplication];
                            ({
                                let value: () = msg_send![app, setServicesMenu: item];
                                value
                            });
                        }
                    }

                    item
                }
            }
        }
    }

    fn os_version() -> Version {
        let version = {
            let process_info = unsafe {
                let value: id = msg_send![class!(NSProcessInfo), processInfo];
                value
            };
            unsafe {
                let value: NSOperatingSystemVersion =
                    msg_send![process_info, operatingSystemVersion];
                value
            }
        };
        Version::new(
            version.majorVersion as u64,
            version.minorVersion as u64,
            version.patchVersion as u64,
        )
    }

    // Before the app finishes launching there is no delegate yet;
    // `did_finish_launching` registers the observers then.
    fn ensure_system_power_observers(&self) {
        if self.0.lock().system_power_observers_registered {
            return;
        }

        // The shared application must be created through `APP_CLASS`, or `run`
        // finds a plain `NSApplication` without the `platform` ivar; only the
        // delegate lookup goes through the typed binding.
        // SAFETY: APP_CLASS is registered during startup and `sharedApplication`
        // returns a live NSApplication instance.
        let delegate = unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            (&*(app as *const objc2_app_kit::NSApplication)).delegate()
        };
        if let Some(delegate) = delegate {
            register_system_power_observers(delegate.as_ref());
            self.0.lock().system_power_observers_registered = true;
        }
    }
}

impl Platform for MacPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.0.lock().background_executor.clone()
    }

    fn foreground_executor(&self) -> gpui::ForegroundExecutor {
        self.0.lock().foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.0.lock().text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
        let mut state = self.0.lock();
        if state.headless {
            drop(state);
            on_finish_launching();
            unsafe { CFRunLoopRun() };
        } else {
            state.finish_launching = Some(on_finish_launching);
            drop(state);
        }

        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            // An accessory app must not register in the Dock during launch, so its
            // policy has to precede the run loop. `Regular` is applied in
            // `did_finish_launching` instead: setting it this early leaves the menu
            // bar of an unbundled app launched from a terminal unclickable.
            let policy = {
                let mut state = self.0.lock();
                state.application_created = true;
                state.activation_policy
            };
            if policy == ActivationPolicy::Accessory {
                ({
                    let value: BOOL =
                        msg_send![app, setActivationPolicy: native_activation_policy(policy)];
                    value
                });
            }
            let app_delegate: id = msg_send![APP_DELEGATE_CLASS, new];
            ({
                let value: () = msg_send![app, setDelegate: app_delegate];
                value
            });

            let self_ptr = self as *const Self as *const c_void;
            (*app).set_ivar(MAC_PLATFORM_IVAR, self_ptr);
            (*app_delegate).set_ivar(MAC_PLATFORM_IVAR, self_ptr);

            let pool = {
                let value: id = msg_send![class!(NSAutoreleasePool), new];
                value
            };
            ({
                let value: () = msg_send![app, run];
                value
            });
            ({
                let value: () = msg_send![pool, drain];
                value
            });

            (*app).set_ivar(MAC_PLATFORM_IVAR, null_mut::<c_void>());
            (*({
                let value: id = msg_send![app, delegate];
                value
            }))
            .set_ivar(MAC_PLATFORM_IVAR, null_mut::<c_void>());
        }
        self.0.lock().application_created = false;
    }

    fn quit(&self) {
        // Quitting the app causes us to close windows, which invokes `Window::on_close` callbacks
        // synchronously before this method terminates. If we call `Platform::quit` while holding a
        // borrow of the app state (which most of the time we will do), we will end up
        // double-borrowing the app state in the `on_close` callbacks for our open windows. To solve
        // this, we make quitting the application asynchronous so that we aren't holding borrows to
        // the app state on the stack when we actually terminate the app.

        unsafe {
            DispatchQueue::main().exec_async_f(ptr::null_mut(), quit);
        }

        extern "C" fn quit(_: *mut c_void) {
            unsafe {
                let app = {
                    let value: id = msg_send![class!(NSApplication), sharedApplication];
                    value
                };
                let _: () = msg_send![app, terminate: nil];
            }
        }
    }

    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<std::ffi::OsString>) {
        use std::os::unix::process::CommandExt as _;

        let app_pid = std::process::id().to_string();
        let app_path = binary_path
            .or_else(|| {
                self.app_path()
                    .ok()
                    // When the app is not bundled, `app_path` returns the
                    // directory containing the executable. Disregard this
                    // and get the path to the executable itself.
                    .and_then(|path| (path.extension()?.to_str()? == "app").then_some(path))
            })
            .unwrap_or_else(|| std::env::current_exe().unwrap());

        // Wait until this process has exited and then re-open this path.
        let script = r#"
            while kill -0 $0 2> /dev/null; do
                sleep 0.1
            done
            app_path="$1"
            shift
            if (($# > 0)); then
                open "$app_path" --args "$@"
            else
                open "$app_path"
            fi
        "#;

        #[allow(
            clippy::disallowed_methods,
            reason = "We are restarting ourselves, using std command thus is fine"
        )]
        let restart_process = new_std_command("/bin/bash")
            .arg("-c")
            .arg(script)
            .arg(app_pid)
            .arg(app_path)
            .args(arguments)
            .process_group(0)
            .spawn();

        match restart_process {
            Ok(_) => self.quit(),
            Err(e) => log::error!("failed to spawn restart script: {:?}", e),
        }
    }

    fn activate(&self, ignoring_other_apps: bool) {
        {
            let app = unsafe {
                let value: id = msg_send![class!(NSApplication), sharedApplication];
                value
            };
            (unsafe {
                let value: () =
                    msg_send![app, activateIgnoringOtherApps: ignoring_other_apps.to_objc()];
                value
            });
        }
    }

    fn set_activation_policy(&self, policy: ActivationPolicy) {
        let mut state = self.0.lock();
        state.activation_policy = policy;
        let should_apply = state.application_created && !state.headless;
        drop(state);
        if should_apply {
            unsafe {
                let app: id = msg_send![APP_CLASS, sharedApplication];
                ({
                    let value: BOOL =
                        msg_send![app, setActivationPolicy: native_activation_policy(policy)];
                    value
                });
            }
        }
    }

    fn set_initial_windowing(&self, request: WindowingRequest) {
        self.set_activation_policy(activation_policy_for(&request));
    }

    fn request_windowing(&self, request: WindowingRequest) -> Task<Result<()>> {
        let state = self.0.lock();
        if state.headless {
            return Task::ready(Err(anyhow!(
                "a platform created headless has no application to switch windowing modes"
            )));
        }
        let policy = activation_policy_for(&request);
        if state.activation_policy == policy {
            return Task::ready(Err(match request {
                WindowingRequest::Headless => anyhow!("already headless"),
                WindowingRequest::Windowed(_) => anyhow!("already windowed"),
            }));
        }
        drop(state);
        self.set_activation_policy(policy);
        Task::ready(Ok(()))
    }

    fn graphical_environment(&self) -> Option<GraphicalEnvironment> {
        let state = self.0.lock();
        (!state.headless && state.activation_policy == ActivationPolicy::Regular)
            .then(GraphicalEnvironment::detect)
    }

    fn hide(&self) {
        unsafe {
            let app = {
                let value: id = msg_send![class!(NSApplication), sharedApplication];
                value
            };
            let _: () = msg_send![app, hide: nil];
        }
    }

    fn hide_other_apps(&self) {
        unsafe {
            let app = {
                let value: id = msg_send![class!(NSApplication), sharedApplication];
                value
            };
            let _: () = msg_send![app, hideOtherApplications: nil];
        }
    }

    fn unhide_other_apps(&self) {
        unsafe {
            let app = {
                let value: id = msg_send![class!(NSApplication), sharedApplication];
                value
            };
            let _: () = msg_send![app, unhideAllApplications: nil];
        }
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(MacDisplay::primary()))
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        MacDisplay::all()
            .map(|screen| Rc::new(screen) as Rc<_>)
            .collect()
    }

    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().on_displays_changed = Some(callback);
    }

    #[cfg(feature = "screen-capture")]
    fn is_screen_capture_supported(&self) -> bool {
        let min_version = operating_system_version(12, 3, 0);
        crate::is_macos_version_at_least(min_version)
    }

    #[cfg(feature = "screen-capture")]
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn gpui::ScreenCaptureSource>>>> {
        crate::screen_capture::get_sources(self.1)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        MacWindow::active_window()
    }

    // Returns the windows ordered front-to-back, meaning that the active
    // window is the first one in the returned vec.
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        Some(MacWindow::ordered_windows())
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        // Native popups are not implemented on macOS yet. Rejecting lets callers fall back to
        // gpui's in-window popovers.
        if let WindowKind::AnchoredPopup(_) = options.kind {
            return Err(PopupNotSupportedError.into());
        }

        let (cursor_visible, foreground_executor, background_executor, renderer_context) = {
            let guard = self.0.lock();
            (
                guard.cursor_visible.clone(),
                guard.foreground_executor.clone(),
                guard.background_executor.clone(),
                guard.renderer_context.clone(),
            )
        };

        Ok(Box::new(MacWindow::open(
            handle,
            options,
            cursor_visible,
            foreground_executor,
            background_executor,
            renderer_context,
            self.1,
        )))
    }

    fn window_appearance(&self) -> WindowAppearance {
        unsafe {
            let app = {
                let value: id = msg_send![class!(NSApplication), sharedApplication];
                value
            };
            let appearance: id = msg_send![app, effectiveAppearance];
            crate::window_appearance::window_appearance_from_native(appearance)
        }
    }

    fn set_window_appearance(&self, appearance: Option<WindowAppearance>) {
        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            // `None` clears the override by setting a nil appearance, so the app
            // falls back to tracking the system-wide light/dark setting.
            let ns_appearance: id = match appearance {
                None => nil,
                Some(appearance) => {
                    let name: id = match appearance {
                        WindowAppearance::Light => crate::window_appearance::NSAppearanceNameAqua,
                        WindowAppearance::Dark => {
                            crate::window_appearance::NSAppearanceNameDarkAqua
                        }
                        WindowAppearance::VibrantLight => {
                            std::ptr::from_ref(objc2_app_kit::NSAppearanceNameVibrantLight)
                                .cast_mut()
                                .cast::<Object>()
                        }
                        WindowAppearance::VibrantDark => {
                            std::ptr::from_ref(objc2_app_kit::NSAppearanceNameVibrantDark)
                                .cast_mut()
                                .cast::<Object>()
                        }
                    };
                    msg_send![class!(NSAppearance), appearanceNamed: name]
                }
            };
            let _: () = msg_send![app, setAppearance: ns_appearance];
        }
    }

    fn open_url(&self, url: &str) {
        unsafe {
            let ns_url = {
                let value: id = msg_send![ ( { let value: id = msg_send![class!(NSURL), alloc]; value }), initWithString: ns_string(url)];
                value
            };
            if ns_url.is_null() {
                log::error!("Failed to create NSURL from string: {}", url);
                return;
            }
            let url = {
                let value: id = msg_send![ns_url, autorelease];
                value
            };
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let _: BOOL = msg_send![workspace, openURL: url];
        }
    }

    fn register_url_scheme(&self, scheme: &str) -> Task<anyhow::Result<()>> {
        use objc2_foundation::{NSBundle, NSError, NSString};

        // API only available post Monterey
        // https://developer.apple.com/documentation/appkit/nsworkspace/3753004-setdefaultapplicationaturl
        let (done_tx, done_rx) = oneshot::channel();
        if Self::os_version() < Version::new(12, 0, 0) {
            return Task::ready(Err(anyhow!(
                "macOS 12.0 or later is required to register URL schemes"
            )));
        }

        let Some(bundle_id) = NSBundle::mainBundle().bundleIdentifier() else {
            return Task::ready(Err(anyhow!("Can only register URL scheme in bundled apps")));
        };

        let workspace = NSWorkspace::sharedWorkspace();
        let Some(app) = workspace.URLForApplicationWithBundleIdentifier(&bundle_id) else {
            return Task::ready(Err(anyhow!(
                "Cannot register URL scheme until app is installed"
            )));
        };

        let scheme = NSString::from_str(scheme);

        let done_tx = Cell::new(Some(done_tx));
        let handler = RcBlock::new(move |error: *mut NSError| {
            let result = if let Some(error) = unsafe { error.as_ref() } {
                Err(anyhow!(
                    "Failed to register: {}",
                    error.localizedDescription()
                ))
            } else {
                Ok(())
            };

            if let Some(done_tx) = done_tx.take() {
                _ = done_tx.send(result);
            }
        });

        workspace.setDefaultApplicationAtURL_toOpenURLsWithScheme_completionHandler(
            &app,
            &scheme,
            Some(&handler),
        );

        self.background_executor().spawn(async { done_rx.await? })
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.0.lock().open_urls = Some(callback);
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        use objc2_foundation::NSString;

        let marker = self.1;
        let (done_tx, done_rx) = oneshot::channel();
        self.foreground_executor()
            .spawn(async move {
                let panel = NSOpenPanel::openPanel(marker);
                panel.setCanChooseDirectories(options.directories);
                panel.setCanChooseFiles(options.files);
                panel.setAllowsMultipleSelection(options.multiple);

                panel.setCanCreateDirectories(true);
                panel.setResolvesAliases(false);

                let done_tx = Cell::new(Some(done_tx));
                let handler = RcBlock::new({
                    let panel = panel.clone();
                    move |response: NSModalResponse| {
                        let Some(done_tx) = done_tx.take() else {
                            return;
                        };

                        let result = (response == NSModalResponseOK).then(|| {
                            panel
                                .URLs()
                                .iter()
                                .filter(|url| url.isFileURL())
                                .filter_map(|url| url.to_file_path())
                                .collect::<Vec<_>>()
                        });
                        _ = done_tx.send(Ok(result));
                    }
                });

                if let Some(prompt) = options.prompt {
                    panel.setPrompt(Some(&NSString::from_str(prompt.as_str())));
                }

                panel.beginWithCompletionHandler(&handler);
            })
            .detach();
        done_rx
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        use objc2_foundation::{NSString, NSURL};

        let url = NSURL::from_directory_path(directory);
        let suggested_name = suggested_name.map(NSString::from_str);
        let (done_tx, done_rx) = oneshot::channel();
        let marker = self.1;
        self.foreground_executor()
            .spawn(async move {
                let panel = NSSavePanel::savePanel(marker);
                panel.setDirectoryURL(url.as_deref());

                if let Some(suggested_name) = suggested_name {
                    panel.setNameFieldStringValue(&suggested_name);
                }

                let done_tx = Cell::new(Some(done_tx));
                let handler = RcBlock::new({
                    let panel = panel.clone();
                    move |response: NSModalResponse| {
                        let Some(done_tx) = done_tx.take() else {
                            return;
                        };

                        let result = if response == NSModalResponseOK {
                            panel
                                .URL()
                                .filter(|url| url.isFileURL())
                                .and_then(|url| url.to_file_path())
                                .map(|mut path| {
                                    let Some(filename) = path.file_name() else {
                                        return path;
                                    };
                                    let chunks = filename
                                        .as_bytes()
                                        .split(|&b| b == b'.')
                                        .collect::<Vec<_>>();

                                    // https://github.com/zed-industries/zed/issues/16969
                                    // Workaround a bug in macOS Sequoia that adds an extra file-extension
                                    // sometimes. e.g. `a.sql` becomes `a.sql.s` or `a.txtx` becomes `a.txtx.txt`
                                    //
                                    // This is conditional on OS version because I'd like to get rid of it, so that
                                    // you can manually create a file called `a.sql.s`. That said it seems better
                                    // to break that use-case than breaking `a.sql`.
                                    if let &[_, second, third] = chunks.as_slice()
                                        && second.starts_with(third)
                                        && Self::os_version() >= Version::new(15, 0, 0)
                                    {
                                        path.set_extension("");
                                    }

                                    path
                                })
                        } else {
                            None
                        };

                        _ = done_tx.send(Ok(result));
                    }
                });

                panel.beginWithCompletionHandler(&handler);
            })
            .detach();

        done_rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        true
    }

    fn reveal_path(&self, path: &Path) {
        unsafe {
            let path = path.to_path_buf();
            self.0
                .lock()
                .background_executor
                .spawn(async move {
                    let full_path = ns_string(path.to_str().unwrap_or(""));
                    let root_full_path = ns_string("");
                    let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
                    let _: BOOL = msg_send![
                        workspace,
                        selectFile: full_path
                        inFileViewerRootedAtPath: root_full_path
                    ];
                })
                .detach();
        }
    }

    fn open_with_system(&self, path: &Path) {
        let path = path.to_owned();
        self.0
            .lock()
            .background_executor
            .spawn(async move {
                #[allow(
                    clippy::disallowed_methods,
                    reason = "running on a background thread, so blocking is fine"
                )]
                new_std_command("open")
                    .arg("--")
                    .arg(path)
                    .status()
                    .context("invoking open command")
                    .log_err();
            })
            .detach();
    }

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.lock().quit = Some(callback);
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().reopen = Some(callback);
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().on_keyboard_layout_change = Some(callback);
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.0.lock().menu_command = Some(callback);
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().will_open_menu = Some(callback);
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.0.lock().validate_menu_command = Some(callback);
    }

    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        let observer = ThermalObserver::new(&self.foreground_executor(), callback);
        self.0.lock().thermal_observer = Some(observer);
    }

    fn on_system_sleep(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().on_system_sleep = Some(callback);
        self.ensure_system_power_observers();
    }

    fn on_system_wake(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().on_system_wake = Some(callback);
        self.ensure_system_power_observers();
    }

    fn thermal_state(&self) -> ThermalState {
        thermal::thermal_state()
    }

    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(Ok(AppleActivity::begin(
            reason,
            NSActivityOptions::UserInitiated,
        )))
    }

    fn show_system_notification(&self, notification: gpui::SystemNotification) {
        let mut state = self.0.lock();
        let executor = state.foreground_executor.clone();
        state.system_notifications.show(&executor, notification);
    }

    fn dismiss_system_notification(&self, tag: &str) {
        let mut state = self.0.lock();
        let executor = state.foreground_executor.clone();
        state.system_notifications.dismiss(&executor, tag);
    }

    fn on_system_notification_response(
        &self,
        callback: Box<dyn FnMut(gpui::SystemNotificationResponse)>,
    ) {
        let mut state = self.0.lock();
        let executor = state.foreground_executor.clone();
        state.system_notifications.on_response(&executor, callback);
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(MacKeyboardLayout::new())
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.0.lock().keyboard_mapper.clone()
    }

    fn app_path(&self) -> Result<PathBuf> {
        unsafe {
            let bundle: id = {
                let bundle: id = msg_send![class!(NSBundle), mainBundle];
                bundle
            };
            anyhow::ensure!(!bundle.is_null(), "app is not running inside a bundle");
            Ok(path_from_objc(msg_send![bundle, bundlePath]))
        }
    }

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            let mut state = self.0.lock();
            let actions = &mut state.menu_actions;
            let menu = self.create_menu_bar(
                &menus,
                {
                    let value: id = msg_send![app, delegate];
                    value
                },
                actions,
                keymap,
            );
            drop(state);
            ({
                let value: () = msg_send![app, setMainMenu: menu];
                value
            });
        }
        self.0.lock().menus = Some(menus.into_iter().map(|menu| menu.owned()).collect());
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.0.lock().menus.clone()
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            let mut state = self.0.lock();
            let actions = &mut state.menu_actions;
            let new = self.create_dock_menu(
                menu,
                {
                    let value: id = msg_send![app, delegate];
                    value
                },
                actions,
                keymap,
            );
            if let Some(old) = state.dock_menu.replace(new) {
                CFRelease(old as _)
            }
        }
    }

    fn add_recent_document(&self, path: &Path) {
        if let Some(path_str) = path.to_str() {
            unsafe {
                let document_controller: id =
                    msg_send![class!(NSDocumentController), sharedDocumentController];
                let url: id = {
                    let value: id = msg_send![class!(NSURL), fileURLWithPath: ns_string(path_str)];
                    value
                };
                let _: () = msg_send![document_controller, noteNewRecentDocumentURL:url];
            }
        }
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        unsafe {
            let bundle: id = {
                let bundle: id = msg_send![class!(NSBundle), mainBundle];
                bundle
            };
            anyhow::ensure!(!bundle.is_null(), "app is not running inside a bundle");
            let name = ns_string(name);
            let url: id = msg_send![bundle, URLForAuxiliaryExecutable: name];
            anyhow::ensure!(!url.is_null(), "resource not found");
            ns_url_to_path(url)
        }
    }

    /// Match cursor style to one of the styles available
    /// in macOS's [NSCursor](https://developer.apple.com/documentation/appkit/nscursor).
    fn set_cursor_style(&self, style: CursorStyle) {
        unsafe {
            set_active_window_cursor_style(style);
        }
    }

    fn hide_cursor_until_mouse_moves(&self) {
        let cursor_visible = self.0.lock().cursor_visible.clone();
        if !cursor_visible.swap(false, Ordering::Relaxed) {
            return;
        }
        unsafe {
            let _: () = msg_send![class!(NSCursor), setHiddenUntilMouseMoves: YES];
        }
    }

    fn is_cursor_visible(&self) -> bool {
        self.0.lock().cursor_visible.load(Ordering::Relaxed)
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        #[allow(non_upper_case_globals)]
        const NSScrollerStyleOverlay: NSInteger = 1;

        unsafe {
            let style: NSInteger = msg_send![class!(NSScroller), preferredScrollerStyle];
            style == NSScrollerStyleOverlay
        }
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        let state = self.0.lock();
        state.general_pasteboard.read()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        let state = self.0.lock();
        state.general_pasteboard.write(item);
    }

    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        let state = self.0.lock();
        state.find_pasteboard.read()
    }

    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        let state = self.0.lock();
        state.find_pasteboard.write(item);
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        let url = url.to_string();
        let username = username.to_string();
        let password = password.to_vec();
        self.background_executor()
            .spawn(async move { Ok(keychain::write_credentials(&url, &username, &password)?) })
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        let url = url.to_string();
        self.background_executor()
            .spawn(async move { Ok(keychain::read_credentials(&url)?) })
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        let url = url.to_string();
        self.background_executor()
            .spawn(async move { Ok(keychain::delete_credentials(&url)?) })
    }
}

unsafe fn path_from_objc(path: id) -> PathBuf {
    let len = msg_send![path, lengthOfBytesUsingEncoding: NSUTF8StringEncoding];
    let bytes = {
        (unsafe {
            let value: *const std::ffi::c_char = msg_send![path, UTF8String];
            value
        }) as *const u8
    };
    let path = str::from_utf8(unsafe { slice::from_raw_parts(bytes, len) }).unwrap();
    PathBuf::from(path)
}

unsafe fn get_mac_platform(object: &mut Object) -> &MacPlatform {
    unsafe {
        let platform_ptr: *mut c_void = *object
            .class()
            .instance_variable(c"platform")
            .expect("GPUI state ivar")
            .load::<*mut c_void>(object);
        assert!(!platform_ptr.is_null());
        &*(platform_ptr as *const MacPlatform)
    }
}

extern "C-unwind" fn will_finish_launching(_this: *mut Object, _: Sel, _: id) {
    let _this = unsafe { &mut *_this };
    unsafe {
        let user_defaults: id = msg_send![class!(NSUserDefaults), standardUserDefaults];

        // The autofill heuristic controller causes slowdown and high CPU usage.
        // We don't know exactly why. This disables the full heuristic controller.
        //
        // Adapted from: https://github.com/ghostty-org/ghostty/pull/8625
        let name = ns_string("NSAutoFillHeuristicControllerEnabled");
        let existing_value: id = msg_send![user_defaults, objectForKey: name];
        if existing_value == nil {
            let false_value: id = msg_send![class!(NSNumber), numberWithBool:false];
            let _: () = msg_send![user_defaults, setObject: false_value forKey: name];
        }
    }
}

extern "C-unwind" fn did_finish_launching(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    unsafe {
        let app: id = msg_send![APP_CLASS, sharedApplication];
        let policy = get_mac_platform(this).0.lock().activation_policy;
        ({
            let value: BOOL = msg_send![app, setActivationPolicy: native_activation_policy(policy)];
            value
        });

        let notification_center: *mut Object =
            msg_send![class!(NSNotificationCenter), defaultCenter];
        let name = ns_string("NSTextInputContextKeyboardSelectionDidChangeNotification");
        let _: () = msg_send![notification_center, addObserver: this as id
            selector: sel!(onKeyboardLayoutChange:)
            name: name
            object: nil
        ];

        // Posted when a display is connected, disconnected, rearranged, or
        // changes mode, which includes its refresh rate.
        let _: () = msg_send![notification_center, addObserver: this as id
            selector: sel!(onScreenParametersChange:)
            name: ns_string("NSApplicationDidChangeScreenParametersNotification")
            object: nil
        ];

        // SAFETY: `this` is a live Objective-C object; only the pointer's type changes.
        let observer = &*(this as *mut Object as *const AnyObject);
        let platform = get_mac_platform(this);
        let callback = {
            let mut state = platform.0.lock();
            if (state.on_system_sleep.is_some() || state.on_system_wake.is_some())
                && !state.system_power_observers_registered
            {
                register_system_power_observers(observer);
                state.system_power_observers_registered = true;
            }
            state.finish_launching.take()
        };
        if let Some(callback) = callback {
            callback();
        }
    }
}

/// `observer` must be the app delegate, whose class declares `onSystemSleep:`
/// and `onSystemWake:` (see `build_classes`).
fn register_system_power_observers(observer: &AnyObject) {
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: both selectors are declared on the delegate class with the
    // notification-handler signature these observers are invoked with.
    unsafe {
        center.addObserver_selector_name_object(
            observer,
            objc2::sel!(onSystemSleep:),
            Some(NSWorkspaceWillSleepNotification),
            None,
        );
        center.addObserver_selector_name_object(
            observer,
            objc2::sel!(onSystemWake:),
            Some(NSWorkspaceDidWakeNotification),
            None,
        );
    }
}

extern "C-unwind" fn should_handle_reopen(
    this: *mut Object,
    _: Sel,
    _: id,
    has_open_windows: BOOL,
) -> BOOL {
    let this = unsafe { &mut *this };
    if !has_open_windows.as_bool() {
        let platform = unsafe { get_mac_platform(this) };
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.reopen.take() {
            drop(lock);
            callback();
            platform.0.lock().reopen.get_or_insert(callback);
        }
    }
    // AppKit requires a BOOL return value for applicationShouldHandleReopen.
    YES
}

extern "C-unwind" fn will_terminate(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    let platform = unsafe { get_mac_platform(this) };
    let mut lock = platform.0.lock();
    if let Some(mut callback) = lock.quit.take() {
        drop(lock);
        callback();
        platform.0.lock().quit.get_or_insert(callback);
    }
}

extern "C-unwind" fn on_keyboard_layout_change(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    let platform = unsafe { get_mac_platform(this) };
    let mut lock = platform.0.lock();
    let keyboard_layout = MacKeyboardLayout::new();
    lock.keyboard_mapper = Rc::new(MacKeyboardMapper::new(keyboard_layout.id()));
    if let Some(mut callback) = lock.on_keyboard_layout_change.take() {
        drop(lock);
        callback();
        platform
            .0
            .lock()
            .on_keyboard_layout_change
            .get_or_insert(callback);
    }
}

extern "C-unwind" fn on_screen_parameters_change(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    // Defer to the next run loop iteration to avoid re-entrant borrows of the App RefCell,
    // as NSNotificationCenter delivers this notification synchronously and it may fire while
    // the App is already borrowed (same pattern as quit() above).
    let platform = unsafe { get_mac_platform(this) };
    let platform_ptr = platform as *const MacPlatform as *mut c_void;
    unsafe {
        DispatchQueue::main().exec_async_f(platform_ptr, on_screen_parameters_change);
    }

    extern "C" fn on_screen_parameters_change(context: *mut c_void) {
        let platform = unsafe { &*(context as *const MacPlatform) };
        let callback = platform.0.lock().on_displays_changed.take();
        if let Some(mut callback) = callback {
            callback();
            platform
                .0
                .lock()
                .on_displays_changed
                .get_or_insert(callback);
        }
    }
}

// Deferred to the main queue like the wake and thermal notifications. The
// machine keeps running the main run loop for well over a frame after the
// observers return, so the block runs before it suspends.
extern "C-unwind" fn on_system_sleep(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    // SAFETY: this is the registered app delegate carrying MAC_PLATFORM_IVAR.
    let platform = unsafe { get_mac_platform(this) };
    let platform_ptr = platform as *const MacPlatform as *mut c_void;
    // SAFETY: platform lives for the process lifetime while callbacks are registered.
    unsafe {
        DispatchQueue::main().exec_async_f(platform_ptr, on_system_sleep);
    }

    extern "C" fn on_system_sleep(context: *mut c_void) {
        // SAFETY: context is the MacPlatform pointer queued above.
        let platform = unsafe { &*(context as *const MacPlatform) };
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.on_system_sleep.take() {
            drop(lock);
            callback();
            platform.0.lock().on_system_sleep.get_or_insert(callback);
        }
    }
}

extern "C-unwind" fn on_system_wake(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    // SAFETY: this is the registered app delegate carrying MAC_PLATFORM_IVAR.
    let platform = unsafe { get_mac_platform(this) };
    let platform_ptr = platform as *const MacPlatform as *mut c_void;
    // SAFETY: platform lives for the process lifetime while callbacks are registered.
    unsafe {
        DispatchQueue::main().exec_async_f(platform_ptr, on_system_wake);
    }

    extern "C" fn on_system_wake(context: *mut c_void) {
        // SAFETY: context is the MacPlatform pointer queued above.
        let platform = unsafe { &*(context as *const MacPlatform) };
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.on_system_wake.take() {
            drop(lock);
            callback();
            platform.0.lock().on_system_wake.get_or_insert(callback);
        }
    }
}

extern "C-unwind" fn open_urls(this: *mut Object, _: Sel, _: id, urls: id) {
    let this = unsafe { &mut *this };
    let urls = unsafe {
        (0..({
            let count: usize = msg_send![urls, count];
            count
        }))
            .filter_map(|i| {
                let url = {
                    let value: id = msg_send![urls, objectAtIndex: i];
                    value
                };
                match CStr::from_ptr(
                    ({
                        let value: *const std::ffi::c_char = msg_send![
                            ({
                                let value: id = msg_send![url, absoluteString];
                                value
                            }),
                            UTF8String
                        ];
                        value
                    }) as *mut c_char,
                )
                .to_str()
                {
                    Ok(string) => Some(string.to_string()),
                    Err(err) => {
                        log::error!("error converting path to string: {}", err);
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
    };
    let platform = unsafe { get_mac_platform(this) };
    let mut lock = platform.0.lock();
    if let Some(mut callback) = lock.open_urls.take() {
        drop(lock);
        callback(urls);
        platform.0.lock().open_urls.get_or_insert(callback);
    }
}

extern "C-unwind" fn handle_menu_item(this: *mut Object, _: Sel, item: id) {
    let this = unsafe { &mut *this };
    unsafe {
        let platform = get_mac_platform(this);
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.menu_command.take() {
            let tag: NSInteger = msg_send![item, tag];
            let index = tag as usize;
            if let Some(action) = lock.menu_actions.get(index) {
                let action = action.boxed_clone();
                drop(lock);
                callback(&*action);
            }
            platform.0.lock().menu_command.get_or_insert(callback);
        }
    }
}

extern "C-unwind" fn validate_menu_item(this: *mut Object, _: Sel, item: id) -> BOOL {
    let this = unsafe { &mut *this };
    unsafe {
        let mut result = false;
        let platform = get_mac_platform(this);
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.validate_menu_command.take() {
            let tag: NSInteger = msg_send![item, tag];
            let index = tag as usize;
            if let Some(action) = lock.menu_actions.get(index) {
                let action = action.boxed_clone();
                drop(lock);
                result = callback(action.as_ref());
            }
            platform
                .0
                .lock()
                .validate_menu_command
                .get_or_insert(callback);
        }
        BOOL::new(result)
    }
}

extern "C-unwind" fn menu_will_open(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    unsafe {
        let platform = get_mac_platform(this);
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.will_open_menu.take() {
            drop(lock);
            callback();
            platform.0.lock().will_open_menu.get_or_insert(callback);
        }
    }
}

extern "C-unwind" fn handle_dock_menu(this: *mut Object, _: Sel, _: id) -> id {
    let this = unsafe { &mut *this };
    unsafe {
        let platform = get_mac_platform(this);
        let state = platform.0.lock();
        if let Some(id) = state.dock_menu {
            id
        } else {
            nil
        }
    }
}

unsafe fn ns_url_to_path(url: id) -> Result<PathBuf> {
    let path: *mut c_char = msg_send![url, fileSystemRepresentation];
    anyhow::ensure!(!path.is_null(), "url is not a file path: {}", unsafe {
        CStr::from_ptr({
            let value: *const std::ffi::c_char = msg_send![
                ({
                    let value: id = msg_send![url, absoluteString];
                    value
                }),
                UTF8String
            ];
            value
        })
        .to_string_lossy()
    });
    Ok(PathBuf::from(OsStr::from_bytes(unsafe {
        CStr::from_ptr(path).to_bytes()
    })))
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub(super) fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut Object;
    pub(super) fn TISCopyCurrentKeyboardInputSource() -> *mut Object;
    pub(super) fn TISGetInputSourceProperty(
        inputSource: *mut Object,
        propertyKey: *const c_void,
    ) -> *mut Object;

    pub(super) fn UCKeyTranslate(
        keyLayoutPtr: *const ::std::os::raw::c_void,
        virtualKeyCode: u16,
        keyAction: u16,
        modifierKeyState: u32,
        keyboardType: u32,
        keyTranslateOptions: u32,
        deadKeyState: *mut u32,
        maxStringLength: usize,
        actualStringLength: *mut usize,
        unicodeString: *mut u16,
    ) -> u32;
    pub(super) fn LMGetKbdType() -> u16;
    pub(super) static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
    pub(super) static kTISPropertyInputSourceID: CFStringRef;
    pub(super) static kTISPropertyLocalizedName: CFStringRef;
    pub(super) static kTISPropertyInputSourceIsASCIICapable: CFStringRef;
    pub(super) static kTISPropertyInputSourceType: CFStringRef;
    pub(super) static kTISTypeKeyboardInputMode: CFStringRef;
}
