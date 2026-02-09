//! Guest SDK for building PrivStack Wasm plugins.
//!
//! Plugin authors use this crate to implement the `Plugin` trait and
//! optional capability traits. The crate handles WIT binding boilerplate
//! and provides ergonomic constructors for entity schemas, indexed fields,
//! and other WIT types.
//!
//! # Example
//!
//! (Requires `wasm32` target — WIT-generated types are not available on host.)
//!
//! ```ignore
//! use privstack_plugin_sdk::prelude::*;
//!
//! struct MyPlugin {
//!     // plugin state
//! }
//!
//! impl Plugin for MyPlugin {
//!     fn metadata(&self) -> PluginMetadata {
//!         PluginMetadata {
//!             id: "community.my-plugin".into(),
//!             name: "My Plugin".into(),
//!             version: "1.0.0".into(),
//!             ..Default::default()
//!         }
//!     }
//!
//!     fn entity_schemas(&self) -> Vec<EntitySchema> {
//!         vec![EntitySchema {
//!             entity_type: "bookmark".into(),
//!             indexed_fields: vec![
//!                 IndexedField::text("/title", true),
//!                 IndexedField::text("/url", false),
//!                 IndexedField::tag("/tags"),
//!             ],
//!             merge_strategy: MergeStrategy::LwwPerField,
//!         }]
//!     }
//!
//!     fn initialize(&mut self) -> bool { true }
//! }
//! ```

pub mod prelude;
pub mod types;

pub use types::*;

// ---- WASM export macro ----
// Generates all WIT boilerplate for a plugin type. Capability traits listed
// in the optional bracket list get real delegation; unlisted ones get stubs.

/// Generate WIT export glue for a plugin type.
///
/// # Usage
///
/// (Requires `wasm32` target — WIT-generated types are not available on host.)
///
/// ```ignore
/// // No optional capabilities:
/// privstack_plugin_sdk::privstack_wasm_export!(MyPlugin);
///
/// // With capabilities:
/// privstack_plugin_sdk::privstack_wasm_export!(MyPlugin, [LinkableItemProvider]);
/// ```
///
/// The plugin type must implement `Default` and `Plugin`.
#[macro_export]
macro_rules! privstack_wasm_export {
    // Entry: no capabilities
    ($plugin_ty:ty) => {
        $crate::privstack_wasm_export!($plugin_ty, []);
    };
    // Entry: with capabilities list
    ($plugin_ty:ty, [$($cap:ident),* $(,)?]) => {
        #[cfg(target_arch = "wasm32")]
        mod wit_gen {
            wit_bindgen::generate!({
                path: "../wit",
                world: "plugin-world",
                generate_all,
            });
        }

        #[cfg(target_arch = "wasm32")]
        mod __pws_exports {
            use super::*;
            use std::sync::Mutex;

            static PLUGIN: Mutex<Option<$plugin_ty>> = Mutex::new(None);

            pub(crate) fn with_plugin<F, R>(f: F) -> R
            where
                F: FnOnce(&$plugin_ty) -> R,
            {
                let mut guard = PLUGIN.lock().unwrap();
                let plugin = guard.get_or_insert_with(<$plugin_ty>::default);
                f(plugin)
            }

            pub(crate) fn with_plugin_mut<F, R>(f: F) -> R
            where
                F: FnOnce(&mut $plugin_ty) -> R,
            {
                let mut guard = PLUGIN.lock().unwrap();
                let plugin = guard.get_or_insert_with(<$plugin_ty>::default);
                f(plugin)
            }

            use crate::wit_gen::privstack::plugin::types as wit_types;
            use crate::wit_gen::exports::privstack::plugin::plugin as wit_plugin;
            use crate::wit_gen::exports::privstack::plugin::linkable_item_provider as wit_linkable;
            use crate::wit_gen::exports::privstack::plugin::deep_link_target as wit_deep_link;
            use crate::wit_gen::exports::privstack::plugin::timer as wit_timer;
            use crate::wit_gen::exports::privstack::plugin::shutdown_aware as wit_shutdown;
            use crate::wit_gen::exports::privstack::plugin::template_data_provider as wit_template_data;

            // Type conversion helpers
            fn to_wit_metadata(m: $crate::PluginMetadata) -> wit_types::PluginMetadata {
                wit_types::PluginMetadata {
                    id: m.id,
                    name: m.name,
                    description: m.description,
                    version: m.version,
                    author: m.author,
                    icon: m.icon,
                    navigation_order: m.navigation_order,
                    category: match m.category {
                        $crate::PluginCategory::Productivity => wit_types::PluginCategory::Productivity,
                        $crate::PluginCategory::Security => wit_types::PluginCategory::Security,
                        $crate::PluginCategory::Communication => wit_types::PluginCategory::Communication,
                        $crate::PluginCategory::Information => wit_types::PluginCategory::Information,
                        $crate::PluginCategory::Utility => wit_types::PluginCategory::Utility,
                        $crate::PluginCategory::Extension => wit_types::PluginCategory::Extension,
                    },
                    can_disable: m.can_disable,
                    is_experimental: m.is_experimental,
                }
            }

            fn to_wit_schema(s: $crate::EntitySchema) -> wit_types::EntitySchema {
                wit_types::EntitySchema {
                    entity_type: s.entity_type,
                    indexed_fields: s
                        .indexed_fields
                        .into_iter()
                        .map(|f| wit_types::IndexedField {
                            field_path: f.field_path,
                            field_type: match f.field_type {
                                $crate::FieldType::Text => wit_types::FieldType::Text,
                                $crate::FieldType::Tag => wit_types::FieldType::Tag,
                                $crate::FieldType::DateTime => wit_types::FieldType::DateTime,
                                $crate::FieldType::Number => wit_types::FieldType::Number,
                                $crate::FieldType::Boolean => wit_types::FieldType::Boolean,
                                $crate::FieldType::Vector => wit_types::FieldType::Vector,
                                $crate::FieldType::Counter => wit_types::FieldType::Counter,
                                $crate::FieldType::Relation => wit_types::FieldType::Relation,
                                $crate::FieldType::Decimal => wit_types::FieldType::Decimal,
                                $crate::FieldType::Json => wit_types::FieldType::Json,
                                $crate::FieldType::Enumeration => wit_types::FieldType::Enumeration,
                                $crate::FieldType::GeoPoint => wit_types::FieldType::GeoPoint,
                                $crate::FieldType::Duration => wit_types::FieldType::Duration,
                            },
                            searchable: f.searchable,
                            vector_dim: f.vector_dim,
                            enum_options: f.enum_options,
                        })
                        .collect(),
                    merge_strategy: match s.merge_strategy {
                        $crate::MergeStrategy::LwwDocument => wit_types::MergeStrategy::LwwDocument,
                        $crate::MergeStrategy::LwwPerField => wit_types::MergeStrategy::LwwPerField,
                        $crate::MergeStrategy::Custom => wit_types::MergeStrategy::Custom,
                    },
                }
            }

            fn to_wit_nav_item(n: $crate::NavigationItem) -> wit_types::NavigationItem {
                wit_types::NavigationItem {
                    id: n.id,
                    display_name: n.display_name,
                    subtitle: n.subtitle,
                    icon: n.icon,
                    tooltip: n.tooltip,
                    order: n.order,
                    show_badge: n.show_badge,
                    badge_count: n.badge_count,
                    shortcut_hint: n.shortcut_hint,
                }
            }

            fn to_wit_command(c: $crate::CommandDefinition) -> wit_types::CommandDefinition {
                wit_types::CommandDefinition {
                    name: c.name,
                    description: c.description,
                    keywords: c.keywords,
                    category: c.category,
                    icon: c.icon,
                }
            }

            fn to_wit_linkable_item(l: $crate::LinkableItem) -> wit_types::LinkableItem {
                wit_types::LinkableItem {
                    id: l.id,
                    link_type: l.link_type,
                    title: l.title,
                    subtitle: l.subtitle,
                    icon: l.icon,
                    modified_at: l.modified_at,
                }
            }

            // Export struct implementing all WIT Guest traits
            pub struct PluginExports;

            impl wit_plugin::Guest for PluginExports {
                fn get_metadata() -> wit_types::PluginMetadata {
                    with_plugin_mut(|p| to_wit_metadata($crate::Plugin::metadata(p)))
                }

                fn get_entity_schemas() -> Vec<wit_types::EntitySchema> {
                    with_plugin_mut(|p| $crate::Plugin::entity_schemas(p).into_iter().map(to_wit_schema).collect())
                }

                fn get_navigation_item() -> Option<wit_types::NavigationItem> {
                    with_plugin(|p| $crate::Plugin::navigation_item(p).map(to_wit_nav_item))
                }

                fn get_commands() -> Vec<wit_types::CommandDefinition> {
                    with_plugin(|p| $crate::Plugin::commands(p).into_iter().map(to_wit_command).collect())
                }

                fn initialize() -> bool {
                    with_plugin_mut(|p| $crate::Plugin::initialize(p))
                }

                fn activate() {
                    with_plugin_mut(|p| $crate::Plugin::activate(p))
                }

                fn deactivate() {
                    with_plugin_mut(|p| $crate::Plugin::deactivate(p))
                }

                fn on_navigated_to() {
                    with_plugin_mut(|p| $crate::Plugin::on_navigated_to(p))
                }

                fn on_navigated_from() {
                    with_plugin_mut(|p| $crate::Plugin::on_navigated_from(p))
                }

                fn dispose() {
                    with_plugin_mut(|p| $crate::Plugin::dispose(p))
                }

                fn get_view_state() -> String {
                    with_plugin(|p| $crate::Plugin::get_view_state(p))
                }

                fn handle_command(name: String, args: String) -> String {
                    with_plugin_mut(|p| $crate::Plugin::handle_command(p, &name, &args))
                }
            }

            // Capability impls — dispatch to helper macros
            $crate::__pws_linkable_impl!(PluginExports, $plugin_ty, [$($cap),*]);
            $crate::__pws_deep_link_impl!(PluginExports, $plugin_ty, [$($cap),*]);
            $crate::__pws_timer_impl!(PluginExports, $plugin_ty, [$($cap),*]);
            $crate::__pws_shutdown_impl!(PluginExports, $plugin_ty, [$($cap),*]);
            $crate::__pws_template_data_impl!(PluginExports, $plugin_ty, [$($cap),*]);
        }

        // Wire up the export! call
        #[cfg(target_arch = "wasm32")]
        use __pws_exports::PluginExports as _PwsPluginExports;
        #[cfg(target_arch = "wasm32")]
        wit_gen::export!(_PwsPluginExports with_types_in wit_gen);
    };
}

// ---- Capability helper macros ----
// Each uses tt-munching to find its flag in the capability list.

#[doc(hidden)]
#[macro_export]
macro_rules! __pws_linkable_impl {
    // Found the flag — real delegation
    ($exports:ident, $plugin_ty:ty, [LinkableItemProvider $(, $rest:ident)*]) => {
        impl wit_linkable::Guest for $exports {
            fn link_type() -> String {
                with_plugin(|p| $crate::LinkableItemProvider::link_type(p).to_string())
            }
            fn link_type_display_name() -> String {
                with_plugin(|p| $crate::LinkableItemProvider::link_type_display_name(p).to_string())
            }
            fn link_type_icon() -> String {
                with_plugin(|p| $crate::LinkableItemProvider::link_type_icon(p).to_string())
            }
            fn search_items(query: String, max_results: u32) -> Vec<wit_types::LinkableItem> {
                with_plugin(|p| {
                    $crate::LinkableItemProvider::search_items(p, &query, max_results)
                        .into_iter()
                        .map(to_wit_linkable_item)
                        .collect()
                })
            }
            fn get_item_by_id(item_id: String) -> Option<wit_types::LinkableItem> {
                with_plugin(|p| {
                    $crate::LinkableItemProvider::get_item_by_id(p, &item_id)
                        .map(to_wit_linkable_item)
                })
            }
        }
    };
    // Skip non-matching flag, keep searching
    ($exports:ident, $plugin_ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__pws_linkable_impl!($exports, $plugin_ty, [$($rest),*]);
    };
    // Empty list — stub
    ($exports:ident, $plugin_ty:ty, []) => {
        impl wit_linkable::Guest for $exports {
            fn link_type() -> String { String::new() }
            fn link_type_display_name() -> String { String::new() }
            fn link_type_icon() -> String { String::new() }
            fn search_items(_query: String, _max_results: u32) -> Vec<wit_types::LinkableItem> { Vec::new() }
            fn get_item_by_id(_item_id: String) -> Option<wit_types::LinkableItem> { None }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __pws_deep_link_impl {
    // Found the flag — real delegation
    ($exports:ident, $plugin_ty:ty, [DeepLinkTarget $(, $rest:ident)*]) => {
        impl wit_deep_link::Guest for $exports {
            fn link_type() -> String {
                with_plugin(|p| $crate::DeepLinkTarget::link_type(p).to_string())
            }
            fn navigate_to_item(item_id: String) {
                with_plugin_mut(|p| $crate::DeepLinkTarget::navigate_to_item(p, &item_id))
            }
        }
    };
    ($exports:ident, $plugin_ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__pws_deep_link_impl!($exports, $plugin_ty, [$($rest),*]);
    };
    ($exports:ident, $plugin_ty:ty, []) => {
        impl wit_deep_link::Guest for $exports {
            fn link_type() -> String { String::new() }
            fn navigate_to_item(_item_id: String) {}
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __pws_timer_impl {
    // Found the flag — real delegation
    ($exports:ident, $plugin_ty:ty, [TimerBehavior $(, $rest:ident)*]) => {
        impl wit_timer::Guest for $exports {
            fn start_timer(item_id: String) {
                with_plugin_mut(|p| $crate::TimerBehavior::start_timer(p, &item_id))
            }
            fn pause_timer() {
                with_plugin_mut(|p| $crate::TimerBehavior::pause_timer(p))
            }
            fn resume_timer() {
                with_plugin_mut(|p| $crate::TimerBehavior::resume_timer(p))
            }
            fn stop_timer() -> wit_types::TimerResult {
                with_plugin_mut(|p| {
                    let r = $crate::TimerBehavior::stop_timer(p);
                    wit_types::TimerResult {
                        item_id: r.item_id,
                        elapsed_ms: r.elapsed_ms,
                    }
                })
            }
            fn get_timer_state() -> wit_types::TimerState {
                with_plugin(|p| {
                    let s = $crate::TimerBehavior::get_timer_state(p);
                    wit_types::TimerState {
                        is_active: s.is_active,
                        is_running: s.is_running,
                        elapsed_ms: s.elapsed_ms,
                        item_title: s.item_title,
                    }
                })
            }
        }
    };
    ($exports:ident, $plugin_ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__pws_timer_impl!($exports, $plugin_ty, [$($rest),*]);
    };
    ($exports:ident, $plugin_ty:ty, []) => {
        impl wit_timer::Guest for $exports {
            fn start_timer(_item_id: String) {}
            fn pause_timer() {}
            fn resume_timer() {}
            fn stop_timer() -> wit_types::TimerResult {
                wit_types::TimerResult { item_id: String::new(), elapsed_ms: 0 }
            }
            fn get_timer_state() -> wit_types::TimerState {
                wit_types::TimerState { is_active: false, is_running: false, elapsed_ms: 0, item_title: None }
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __pws_shutdown_impl {
    // Found the flag — real delegation
    ($exports:ident, $plugin_ty:ty, [ShutdownAware $(, $rest:ident)*]) => {
        impl wit_shutdown::Guest for $exports {
            fn on_shutdown() {
                with_plugin_mut(|p| $crate::ShutdownAware::on_shutdown(p))
            }
        }
    };
    ($exports:ident, $plugin_ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__pws_shutdown_impl!($exports, $plugin_ty, [$($rest),*]);
    };
    ($exports:ident, $plugin_ty:ty, []) => {
        impl wit_shutdown::Guest for $exports {
            fn on_shutdown() {}
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __pws_template_data_impl {
    // Found the flag — real delegation
    ($exports:ident, $plugin_ty:ty, [TemplateDataProvider $(, $rest:ident)*]) => {
        impl wit_template_data::Guest for $exports {
            fn get_view_data() -> String {
                with_plugin(|p| $crate::TemplateDataProvider::get_view_data(p))
            }
        }
    };
    // Skip non-matching flag, keep searching
    ($exports:ident, $plugin_ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__pws_template_data_impl!($exports, $plugin_ty, [$($rest),*]);
    };
    // Empty list — stub
    ($exports:ident, $plugin_ty:ty, []) => {
        impl wit_template_data::Guest for $exports {
            fn get_view_data() -> String { "{}".to_string() }
        }
    };
}
