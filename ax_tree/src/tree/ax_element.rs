//! Minimal wrapper around AXUIElement from objc2-application-services
//!
//! This provides just the functionality we need for building accessibility trees,
//! without pulling in the full cubby-core dependency.

#[cfg(target_os = "macos")]
use libc::pid_t;
#[cfg(target_os = "macos")]
use objc2_application_services::{AXError, AXUIElement};
#[cfg(target_os = "macos")]
use objc2_core_foundation::{CFArray, CFBoolean, CFRetained, CFString, CFType, Type};
#[cfg(target_os = "macos")]
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::ptr::NonNull;

/// Minimal wrapper around AXUIElement for tree building
#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
pub struct AxElement {
    element: CFRetained<AXUIElement>,
    pid: Option<i32>,
}

#[cfg(target_os = "macos")]
impl AxElement {
    /// Create a new AxElement from an AXUIElement
    pub fn new(element: CFRetained<AXUIElement>, pid: Option<i32>) -> Self {
        let resolved_pid = pid.or_else(|| get_element_pid(element.as_ref()));
        Self {
            element,
            pid: resolved_pid,
        }
    }

    /// Get the system-wide root element
    pub fn system_wide() -> Result<Self, BuildElementError> {
        let element = unsafe { AXUIElement::new_system_wide() };
        Ok(Self::new(element, None))
    }

    /// Get an application-specific element for the given PID
    pub fn for_application(pid: i32) -> Result<Self, BuildElementError> {
        let element = unsafe { AXUIElement::new_application(pid as libc::pid_t) };
        Ok(Self::new(element, Some(pid)))
    }

    /// Get the AXUIElement (for advanced operations)
    pub fn as_ax_element(&self) -> &AXUIElement {
        &self.element
    }

    /// Get the role of this element
    pub fn role(&self) -> Result<String, BuildElementError> {
        get_string_attribute(&self.element, "AXRole")
    }

    /// Get the label (title) of this element
    pub fn label(&self) -> Option<String> {
        get_string_attribute(&self.element, "AXTitle")
            .or_else(|_| get_string_attribute(&self.element, "AXLabel"))
            .ok()
    }

    /// Get the value of this element
    pub fn value(&self) -> Option<String> {
        get_string_attribute(&self.element, "AXValue").ok()
    }

    /// Get the description of this element
    pub fn description(&self) -> Option<String> {
        get_string_attribute(&self.element, "AXDescription").ok()
    }

    /// Get the children of this element
    /// For application elements, first tries to get windows, then falls back to regular children
    pub fn children(&self) -> Result<Vec<AxElement>, BuildElementError> {
        let mut all_children = Vec::new();
        let mut seen = HashSet::new();
        let parent_pid = self.pid;

        fn handle_child(
            child_ptr: *const AXUIElement,
            seen: &mut HashSet<usize>,
            all_children: &mut Vec<AxElement>,
            parent_pid: Option<i32>,
        ) {
            if let Some(non_null) = NonNull::new(child_ptr as *mut AXUIElement) {
                let key = non_null.as_ptr() as usize;
                if seen.insert(key) {
                    let child_element = unsafe { (*non_null.as_ptr()).retain() };
                    all_children.push(AxElement::new(child_element, parent_pid));
                }
            }
        }

        fn collect_array_attr(
            element: &AXUIElement,
            attr: &str,
            seen: &mut HashSet<usize>,
            all_children: &mut Vec<AxElement>,
            parent_pid: Option<i32>,
        ) -> bool {
            if let Ok(array) = get_cf_array_attribute(element, attr) {
                let count = array.count();
                for idx in 0..(count as usize) {
                    let child_ref = unsafe { array.value_at_index(idx as _) };
                    handle_child(
                        child_ref as *const AXUIElement,
                        seen,
                        all_children,
                        parent_pid,
                    );
                }
                true
            } else {
                false
            }
        }

        fn collect_single_attr(
            element: &AXUIElement,
            attr: &str,
            seen: &mut HashSet<usize>,
            all_children: &mut Vec<AxElement>,
            parent_pid: Option<i32>,
        ) -> bool {
            if let Ok(value) = get_attribute_value(element, attr) {
                if let Some(element_ref) = value.downcast_ref::<AXUIElement>() {
                    handle_child(element_ref as *const _, seen, all_children, parent_pid);
                    return true;
                }
            }
            false
        }

        // Windows and main/focused windows first
        collect_array_attr(
            &self.element,
            "AXWindows",
            &mut seen,
            &mut all_children,
            parent_pid,
        );
        collect_single_attr(
            &self.element,
            "AXMainWindow",
            &mut seen,
            &mut all_children,
            parent_pid,
        );
        collect_single_attr(
            &self.element,
            "AXFocusedWindow",
            &mut seen,
            &mut all_children,
            parent_pid,
        );
        collect_single_attr(
            &self.element,
            "AXFocusedUIElement",
            &mut seen,
            &mut all_children,
            parent_pid,
        );

        // Standard children and additional navigation/visibility sets
        let mut found_children = collect_array_attr(
            &self.element,
            "AXChildren",
            &mut seen,
            &mut all_children,
            parent_pid,
        );
        for attr in [
            "AXChildrenInNavigationOrder",
            "AXVisibleChildren",
            "AXRemoteChildren",
        ] {
            if collect_array_attr(
                &self.element,
                attr,
                &mut seen,
                &mut all_children,
                parent_pid,
            ) {
                found_children = true;
            }
        }

        if !found_children && all_children.is_empty() {
            return Ok(Vec::new());
        }

        Ok(all_children)
    }

    /// Get the bounds (x, y, width, height) of this element
    pub fn bounds(&self) -> Result<(f64, f64, f64, f64), BuildElementError> {
        use std::os::raw::c_void;

        let mut x = 0.0;
        let mut y = 0.0;
        let mut width = 0.0;
        let mut height = 0.0;

        // Get position
        if let Ok(pos_value) = get_ax_value_attribute(&self.element, "AXPosition") {
            unsafe {
                // Constants from Apple's AXValue.h
                const K_AXVALUE_CGPOINT_TYPE: u32 = 1;

                let value_ref = pos_value.as_ref() as *const CFType as *const c_void;

                #[link(name = "ApplicationServices", kind = "framework")]
                extern "C" {
                    fn AXValueGetValue(value: *const c_void, type_: u32, ptr: *mut c_void) -> i32;
                }

                let mut point = CGPoint { x: 0.0, y: 0.0 };
                let point_ptr = &mut point as *mut CGPoint as *mut c_void;

                if AXValueGetValue(value_ref, K_AXVALUE_CGPOINT_TYPE, point_ptr) != 0 {
                    x = point.x;
                    y = point.y;
                }
            }
        }

        // Get size
        if let Ok(size_value) = get_ax_value_attribute(&self.element, "AXSize") {
            unsafe {
                // Constants from Apple's AXValue.h
                const K_AXVALUE_CGSIZE_TYPE: u32 = 2;

                let value_ref = size_value.as_ref() as *const CFType as *const c_void;

                #[link(name = "ApplicationServices", kind = "framework")]
                extern "C" {
                    fn AXValueGetValue(value: *const c_void, type_: u32, ptr: *mut c_void) -> i32;
                }

                let mut size = CGSize {
                    width: 0.0,
                    height: 0.0,
                };
                let size_ptr = &mut size as *mut CGSize as *mut c_void;

                if AXValueGetValue(value_ref, K_AXVALUE_CGSIZE_TYPE, size_ptr) != 0 {
                    width = size.width;
                    height = size.height;
                }
            }
        }

        Ok((x, y, width, height))
    }

    /// Check if this element is focused
    pub fn is_focused(&self) -> bool {
        get_bool_attribute(&self.element, "AXFocused").unwrap_or(false)
    }

    /// Get a stable ID for this element
    pub fn id(&self) -> Option<String> {
        // Try AXIdentifier first
        get_string_attribute(&self.element, "AXIdentifier")
            .ok()
            .or_else(|| {
                // Fallback: use a hash of role, label, and position
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();

                if let Ok(role) = self.role() {
                    role.hash(&mut hasher);
                }
                if let Some(label) = self.label() {
                    label.hash(&mut hasher);
                }
                if let Ok((x, y, _, _)) = self.bounds() {
                    x.to_bits().hash(&mut hasher);
                    y.to_bits().hash(&mut hasher);
                }

                Some(format!("ax_{:x}", hasher.finish()))
            })
    }

    /// Get all properties as a HashMap
    pub fn properties(&self) -> std::collections::HashMap<String, Option<serde_json::Value>> {
        let mut props = std::collections::HashMap::new();

        // Get all attribute names
        if let Ok(attr_names) = get_cf_array_attribute(&self.element, "AXAttributeNames") {
            let count = attr_names.count();

            for idx in 0..(count as usize) {
                let attr_ref = unsafe { attr_names.value_at_index(idx as _) };

                let attr_name_cf = unsafe {
                    if let Some(non_null) = NonNull::new(attr_ref as *mut CFType) {
                        CFRetained::from_raw(non_null)
                    } else {
                        continue;
                    }
                };
                if let Some(attr_name) = attr_name_cf.downcast_ref::<CFString>() {
                    let name = attr_name.to_string();

                    // Get the attribute value
                    let value = get_attribute_value(&self.element, &name)
                        .ok()
                        .and_then(|v| convert_cf_type_to_json(&v));

                    props.insert(name, value);
                }
            }
        }

        props
    }

    /// Get the process ID if available
    pub fn pid(&self) -> Option<i32> {
        self.pid
    }
}

/// Error type for element operations
#[cfg(target_os = "macos")]
#[derive(Debug, thiserror::Error)]
pub enum BuildElementError {
    #[error("AX API error: {0:?}")]
    AxError(AXError),
    #[error("attribute not found: {0}")]
    AttributeNotFound(String),
    #[error("invalid attribute type")]
    InvalidAttributeType,
    #[error("failed to get children: {0}")]
    GetChildrenFailed(String),
}

// Helper functions

#[cfg(target_os = "macos")]
fn get_string_attribute(
    element: &AXUIElement,
    attr_name: &str,
) -> Result<String, BuildElementError> {
    let attr_cf_string = CFString::from_str(attr_name);
    let mut value_ptr: *const CFType = std::ptr::null();
    let value_out = NonNull::new(&mut value_ptr as *mut *const CFType as *mut *const CFType)
        .ok_or_else(|| BuildElementError::AttributeNotFound(attr_name.to_string()))?;

    unsafe {
        let error = element.copy_attribute_value(&attr_cf_string, value_out);
        if error != AXError::Success {
            return Err(BuildElementError::AxError(error));
        }
    }

    let value = unsafe {
        let non_null = NonNull::new(value_ptr as *mut CFType)
            .ok_or_else(|| BuildElementError::InvalidAttributeType)?;
        CFRetained::from_raw(non_null)
    };
    if let Some(cf_string) = value.downcast_ref::<CFString>() {
        Ok(cf_string.to_string())
    } else {
        // Sometimes the value might be wrapped differently, return NoValue error
        Err(BuildElementError::AxError(AXError::NoValue))
    }
}

#[cfg(target_os = "macos")]
fn get_bool_attribute(element: &AXUIElement, attr_name: &str) -> Result<bool, BuildElementError> {
    let attr_cf_string = CFString::from_str(attr_name);
    let mut value_ptr: *const CFType = std::ptr::null();
    let value_out = NonNull::new(&mut value_ptr as *mut *const CFType as *mut *const CFType)
        .ok_or_else(|| BuildElementError::AttributeNotFound(attr_name.to_string()))?;

    unsafe {
        let error = element.copy_attribute_value(&attr_cf_string, value_out);
        if error != AXError::Success {
            return Err(BuildElementError::AxError(error));
        }
    }

    let value = unsafe {
        let non_null = NonNull::new(value_ptr as *mut CFType)
            .ok_or_else(|| BuildElementError::InvalidAttributeType)?;
        CFRetained::from_raw(non_null)
    };
    if let Some(cf_bool) = value.downcast_ref::<CFBoolean>() {
        Ok(cf_bool.as_bool())
    } else {
        Err(BuildElementError::InvalidAttributeType)
    }
}

#[cfg(target_os = "macos")]
fn get_cf_array_attribute(
    element: &AXUIElement,
    attr_name: &str,
) -> Result<CFRetained<CFArray>, BuildElementError> {
    let attr_cf_string = CFString::from_str(attr_name);
    let mut value_ptr: *const CFType = std::ptr::null();
    let value_out = NonNull::new(&mut value_ptr as *mut *const CFType as *mut *const CFType)
        .ok_or_else(|| BuildElementError::AttributeNotFound(attr_name.to_string()))?;

    unsafe {
        let error = element.copy_attribute_value(&attr_cf_string, value_out);
        if error != AXError::Success {
            return Err(BuildElementError::AxError(error));
        }
    }

    let value = unsafe {
        let non_null = NonNull::new(value_ptr as *mut CFType)
            .ok_or_else(|| BuildElementError::InvalidAttributeType)?;
        CFRetained::from_raw(non_null)
    };
    // Check if it's a CFArray by trying to downcast
    if let Some(_) = value.downcast_ref::<CFArray>() {
        // value_ptr is already pointing to a CFArray, create CFRetained from it
        let array_raw = value_ptr as *const CFArray;
        unsafe {
            let array_ref = &*array_raw;
            Ok(array_ref.retain())
        }
    } else {
        Err(BuildElementError::InvalidAttributeType)
    }
}

#[cfg(target_os = "macos")]
fn get_ax_value_attribute(
    element: &AXUIElement,
    attr_name: &str,
) -> Result<CFRetained<CFType>, BuildElementError> {
    let attr_cf_string = CFString::from_str(attr_name);
    let mut value_ptr: *const CFType = std::ptr::null();
    let value_out = NonNull::new(&mut value_ptr as *mut *const CFType as *mut *const CFType)
        .ok_or_else(|| BuildElementError::AttributeNotFound(attr_name.to_string()))?;

    unsafe {
        let error = element.copy_attribute_value(&attr_cf_string, value_out);
        if error != AXError::Success {
            return Err(BuildElementError::AxError(error));
        }
    }

    Ok(unsafe {
        let non_null = NonNull::new(value_ptr as *mut CFType)
            .ok_or_else(|| BuildElementError::InvalidAttributeType)?;
        CFRetained::from_raw(non_null)
    })
}

#[cfg(target_os = "macos")]
fn get_attribute_value(
    element: &AXUIElement,
    attr_name: &str,
) -> Result<CFRetained<CFType>, BuildElementError> {
    let attr_cf_string = CFString::from_str(attr_name);
    let mut value_ptr: *const CFType = std::ptr::null();
    let value_out = NonNull::new(&mut value_ptr as *mut *const CFType as *mut *const CFType)
        .ok_or_else(|| BuildElementError::AttributeNotFound(attr_name.to_string()))?;

    unsafe {
        let error = element.copy_attribute_value(&attr_cf_string, value_out);
        if error != AXError::Success {
            return Err(BuildElementError::AxError(error));
        }
    }

    Ok(unsafe {
        let non_null = NonNull::new(value_ptr as *mut CFType)
            .ok_or_else(|| BuildElementError::InvalidAttributeType)?;
        CFRetained::from_raw(non_null)
    })
}

#[cfg(target_os = "macos")]
fn get_element_pid(element: &AXUIElement) -> Option<i32> {
    unsafe {
        let mut pid: pid_t = 0;
        let status = AXUIElementGetPid(element, &mut pid);
        if status == AXError::Success && pid != 0 {
            Some(pid as i32)
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementGetPid(element: *const AXUIElement, pid: *mut pid_t) -> AXError;
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
fn convert_cf_type_to_json(value: &CFType) -> Option<serde_json::Value> {
    // Convert CFType to JSON value
    // This is a simplified implementation
    if let Some(cf_string) = value.downcast_ref::<CFString>() {
        Some(serde_json::Value::String(cf_string.to_string()))
    } else if let Some(cf_bool) = value.downcast_ref::<CFBoolean>() {
        Some(serde_json::Value::Bool(cf_bool.as_bool()))
    } else {
        // For other types, convert to string representation
        Some(serde_json::Value::String(format!("{:?}", value)))
    }
}
