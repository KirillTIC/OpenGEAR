use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use ash::{Entry, Instance, vk};
use std::ffi::{CString};

const VALIDATION_LAYERS: &[&str] = &["VK_LAYER_KHRONOS_validation"];
const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);

const DEVICE_EXTENSIONS: &[&str] = &["VK_KHR_swapchain"];

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

fn check_validation_layer_support(entry: &Entry) -> bool {
    let available_layers = unsafe {
        entry.enumerate_instance_layer_properties().unwrap()
    };
    for required in VALIDATION_LAYERS {
        let found = available_layers.iter().any(|layer| {
            let name = unsafe {
                std::ffi::CStr::from_ptr(layer.layer_name.as_ptr())
            };
            name.to_str().unwrap() == *required
        });

        if !found {
            println!("Failed to found validition layer: {}", required);
            return false;
        }
    }

    true
}
fn get_required_extensions(window: &Window) -> Vec<*const i8> {
    let mut extensions: Vec<*const i8> = ash_window::enumerate_required_extensions(
        window.display_handle().unwrap().as_raw()
    ).unwrap().to_vec();
    if ENABLE_VALIDATION_LAYERS {
        extensions.push(ash::ext::debug_utils::NAME.as_ptr());
    }

    extensions
}

fn check_device_extension_support(
    instance: &Instance,
    device: vk::PhysicalDevice,
) -> bool {
    let available = unsafe {
        instance.enumerate_device_extension_properties(device)
            .unwrap()
    };
    
    DEVICE_EXTENSIONS.iter().all(|required| {
        available.iter().any(|ext| {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            name.to_str().unwrap() == *required
        })
    })
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    msg_type: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void
) -> vk::Bool32 {
    let message = std::ffi::CStr::from_ptr((*data).p_message);
    let severity_str = if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        "ERROR"
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        "WARNING"
    } else {
        "INFO"
    };
    
    eprintln!("[Vulkan][{}] {:?}", severity_str, message);

    vk::FALSE
}
fn make_debug_messenger_create_info() -> vk::DebugUtilsMessengerCreateInfoEXT<'static> {
    vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING |
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL     |
            vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION  |
            vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(debug_callback))
}

struct QueueFamilyIndices {
    graphic_family: Option<u32>,
    present_family: Option<u32>,
}
impl QueueFamilyIndices {
    fn is_complete(&self) -> bool {
        self.graphic_family.is_some() && self.present_family.is_some()
    }
}
fn find_queue_families(
    instance: &Instance,
    device: vk::PhysicalDevice,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> QueueFamilyIndices {
    let mut indicies = QueueFamilyIndices {
        graphic_family: None,
        present_family: None
    };
    let queue_families = unsafe {
        instance.get_physical_device_queue_family_properties(device)
    };
    
    for (i, family) in queue_families.iter().enumerate() {
        let i = i as u32;
        
        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            indicies.graphic_family = Some(i);
        }
        
        let present_support = unsafe {
            surface_loader
                .get_physical_device_surface_support(device, i, surface)
                .unwrap_or(false)
        };
        
        if present_support {
            indicies.present_family = Some(i);
        }
        
        if indicies.is_complete() {
            break;
        }
    }
    
    indicies
}
fn is_device_suitable(
    instance: &Instance,
    device: vk::PhysicalDevice,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> bool {
    let properties = unsafe {
        instance.get_physical_device_properties(device)
    };
    let _features = unsafe {
        instance.get_physical_device_features(device)
    };
    let name = unsafe {
        std::ffi::CStr::from_ptr(properties.device_name.as_ptr())
    };
    let extensions_supported = check_device_extension_support(instance, device);
    
    let swap_chain_adequate = if extensions_supported {
        let support = query_swap_chain_support(device, surface_loader, surface);
        !support.formats.is_empty() && !support.present_modes.is_empty()
    } else {
        false
    };
    
    println!("Checking device: {}", name.to_str().unwrap());
    
    find_queue_families(instance, device, surface_loader, surface).is_complete() 
        && extensions_supported 
        && swap_chain_adequate;
    let indices = find_queue_families(instance, device, surface_loader, surface);
    indices.is_complete()
}

struct SwapChainSupportDetails {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}
fn query_swap_chain_support(
    device: vk::PhysicalDevice,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) ->SwapChainSupportDetails {
    unsafe {
        let capabilities = surface_loader
            .get_physical_device_surface_capabilities(device, surface)
            .unwrap();
        let formats = surface_loader
            .get_physical_device_surface_formats(device, surface)
            .unwrap();
        let present_modes = surface_loader
            .get_physical_device_surface_present_modes(device, surface)
            .unwrap();
            
        SwapChainSupportDetails { capabilities, formats, present_modes }
    }
}
fn choose_swap_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    *formats.iter().find(|f| {
        f.format == vk::Format::B8G8R8A8_SRGB
            && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    }).unwrap_or(&formats[0])
}
fn choose_swap_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    if modes.contains(&vk::PresentModeKHR::MAILBOX) {
        vk::PresentModeKHR::MAILBOX
    } else {
        vk::PresentModeKHR::FIFO
    }
}
fn choose_swap_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    window: &winit::window::Window,
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        return capabilities.current_extent;
    }
    let size = window.inner_size();
    vk::Extent2D {
        width: size.width.clamp(
            capabilities.min_image_extent.width,
            capabilities.max_image_extent.width,            
        ),
        height: size.height.clamp(
            capabilities.min_image_extent.height,
            capabilities.max_image_extent.height,
        )
    }
}

#[derive(Default)]
struct App {
    window: Option<Window>,
    entry: Option<Entry>,
    instance: Option<Instance>,
    physical_device: Option<vk::PhysicalDevice>,
    device: Option<ash::Device>,
    surface_loader: Option<ash::khr::surface::Instance>,
    surface: Option<vk::SurfaceKHR>,
    
    graphics_queue: Option<vk::Queue>,
    present_queue: Option<vk::Queue>,
    
    debug_utils_loader: Option<ash::ext::debug_utils::Instance>,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    
    swapchain_loader: Option<ash::khr::swapchain::Device>,
    swapchain: Option<vk::SwapchainKHR>,
    swapchain_images: Vec<vk::Image>,
    swapchain_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_image_views: Vec<vk::ImageView>,
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
            .with_title("Vulkan")
            .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
            .with_resizable(false);

        let window = event_loop.create_window(window_attrs).unwrap();
        self.create_instance(&window);
        self.setup_debug_messenger();
        self.create_surface(&window);
        self.pick_physical_device();
        self.create_logical_device();
        self.create_swap_chain(&window);
        self.create_image_views();
        self.window = Some(window);
        self.init_vulkan();
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            let device = self.device.as_ref().unwrap();
            for &image_view in &self.swapchain_image_views {
                device.destroy_image_view(image_view, None);
            }
            if let (Some(loader), Some(swapchain)) = (
                self.swapchain_loader.take(),
                self.swapchain.take()
            ) {
                loader.destroy_swapchain(swapchain, None);
            }
            if let Some(device) = self.device.take() {
                device.destroy_device(None);
            }
            if let (Some(loader), Some(surface)) = (
                self.surface_loader.take(),
                self.surface.take()
            ) {
                loader.destroy_surface(surface, None);
            }
            if let (Some(loader), Some(messenger)) = (
                self.debug_utils_loader.take(),
                self.debug_messenger.take()
            ) {
                loader.destroy_debug_utils_messenger(messenger, None);
            }
            
            if let Some(instance) = self.instance.take() {
                instance.destroy_instance(None);
            }
        }
    }
}
impl App {
    fn setup_debug_messenger(&mut self) {
        if !ENABLE_VALIDATION_LAYERS { return; }
        let instance = self.instance.as_ref().unwrap();
        let entry = self.entry.as_ref().unwrap();
        let loader = ash::ext::debug_utils::Instance::new(entry, instance);
        let create_info = make_debug_messenger_create_info();
        let messenger = unsafe {
            loader.create_debug_utils_messenger(&create_info, None)
                .expect("Failed to create debug messenger!")
        };

        self.debug_utils_loader = Some(loader);
        self.debug_messenger = Some(messenger);
    }
    fn init_vulkan(&mut self) {
        
    }
    fn cleanup(&mut self) {
        
    }
    fn pick_physical_device(&mut self) {
        let instance = self.instance.as_ref().unwrap();
        let surface_loader = self.surface_loader.as_ref().unwrap();
        let surface = self.surface.unwrap();
        let devices = unsafe {
            instance.enumerate_physical_devices()
                .expect("Failed to get list of devices!")
        };
        
        if devices.is_empty() {
            panic!("No Vulkan-supporting GPUs found!");
        }
        
        let physical_device = devices
            .iter()
            .find(|&&device| is_device_suitable(instance, device, surface_loader, surface))
            .expect("No suitable GPU found!");
        let properties = unsafe {
            instance.get_physical_device_properties(*physical_device)
        };
        let name = unsafe {
            std::ffi::CStr::from_ptr(properties.device_name.as_ptr())
        };
        println!("Selected GPU: {}", name.to_str().unwrap());
        
        self.physical_device = Some(*physical_device)
    }
    fn create_logical_device(&mut self) {
        let instance = self.instance.as_ref().unwrap();
        let physical_device = self.physical_device.unwrap();
        let surface_loader = self.surface_loader.as_ref().unwrap();
        let surface = self.surface.unwrap();
        let indices = find_queue_families(instance, physical_device, surface_loader, surface);
        let graphics_family = indices.graphic_family.unwrap();
        let present_family = indices.present_family.unwrap();
        
        use std::collections::HashSet;
        let unique_families: HashSet<u32> = [graphics_family, present_family]
            .iter()
            .cloned()
            .collect();
        
        let queue_priorities = [1.0f32];
        let queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = unique_families
            .iter()
            .map(|&family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&queue_priorities)
            })
            .collect();
        let device_features = vk::PhysicalDeviceFeatures::default();
        let layer_names: Vec<std::ffi::CString> = VALIDATION_LAYERS.iter()
            .map(|s| std::ffi::CString::new(*s).unwrap())
            .collect();
        let layer_ptrs: Vec<*const i8> = layer_names.iter()
            .map(|s| s.as_ptr())
            .collect();
        let device_extension_names: Vec<std::ffi::CString> = DEVICE_EXTENSIONS.iter()
            .map(|s| std::ffi::CString::new(*s).unwrap())
            .collect();
        let device_extension_ptrs: Vec<*const i8> = device_extension_names.iter()
            .map(|s| s.as_ptr())
            .collect();
        let mut create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_features(&device_features)
            .enabled_extension_names(&device_extension_ptrs);
            
        
        if ENABLE_VALIDATION_LAYERS {
            create_info = create_info.enabled_layer_names(&layer_ptrs);
        }
        
        let device = unsafe {
            instance.create_device(physical_device, &create_info, None)
                .expect("Failed to create logical device!")
        };
        
        let graphics_queue = unsafe {
            device.get_device_queue(graphics_family, 0)
        };
        let present_queue = unsafe {
            device.get_device_queue(present_family, 0)
        };
        
        println!("Logical device successfully created!");
        
        self.device = Some(device);
        self.graphics_queue = Some(graphics_queue);
        self.present_queue = Some(present_queue);
    }
    fn create_instance(&mut self, window: &Window) {
        let entry = unsafe { Entry::load().expect("Failed to load Vulkan!") };
        let app_name = CString::new("OpenGEAR").unwrap();
        let engine_name = CString::new("OpenGEAR Engine").unwrap();
        
        if ENABLE_VALIDATION_LAYERS && !check_validation_layer_support(&entry) {
            panic!("Validation layers are not available!");
        }
        
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_0);
            
        let surface_extensions = ash_window::enumerate_required_extensions(
            window.display_handle().unwrap().as_raw()
        ).unwrap();
        let mut extensions: Vec<*const i8> = surface_extensions.to_vec();
        let extensions = get_required_extensions(window);
        
        let layer_names: Vec<CString> = VALIDATION_LAYERS.iter()
            .map(|s| CString::new(*s).unwrap())
            .collect();
        let layer_ptrs: Vec<*const i8> = layer_names.iter()
            .map(|s| s.as_ptr())
            .collect();
        
        let mut debug_create_info = make_debug_messenger_create_info();
            
        let mut create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions);
            
        if ENABLE_VALIDATION_LAYERS {
        create_info = create_info
            .enabled_layer_names(&layer_ptrs)
            .push_next(&mut debug_create_info);
        }
            
        let instance = unsafe {
            entry.create_instance(&create_info, None)
                .expect("Failed to create instance of Vulkan!")
        };
        
        println!("Vulkan instance successfully created!");
        
        self.entry = Some(entry);
        self.instance = Some(instance);
    }
    fn create_surface(&mut self, window: &Window) {
        let entry = self.entry.as_ref().unwrap();
        let instance = self.instance.as_ref().unwrap();
        
        let surface = unsafe {
            ash_window::create_surface(
                entry,
                instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None
            ).expect("Failed to create surface!")
        };
        
        let surface_loader = ash::khr::surface::Instance::new(entry, instance);
        
        println!("Surface successfully created!");
        
        self.surface_loader = Some(surface_loader);
        self.surface = Some(surface);
    }
    fn create_swap_chain(&mut self, window: &winit::window::Window) {
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let physical_device = self.physical_device.unwrap();
        let surface_loader = self.surface_loader.as_ref().unwrap();
        let surface = self.surface.unwrap();
        let support = query_swap_chain_support(physical_device, surface_loader, surface);
        let format = choose_swap_surface_format(&support.formats);
        let present_mode = choose_swap_present_mode(&support.present_modes);
        let extent = choose_swap_extent(&support.capabilities, window);
        let mut image_count = support.capabilities.min_image_count + 1;
        
        if support.capabilities.max_image_count > 0 {
            image_count = image_count.min(support.capabilities.max_image_count);
        }
        
        let indices = find_queue_families(instance, physical_device, surface_loader, surface);
        let graphics_family = indices.graphic_family.unwrap();
        let present_family = indices.present_family.unwrap();
        let (sharing_mode, queue_family_indicies): (vk::SharingMode, Vec<u32>) = 
            if graphics_family != present_family {
                (vk::SharingMode::CONCURRENT, vec![graphics_family, present_family])
            } else {
                (vk::SharingMode::EXCLUSIVE, vec![])
            };
        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(sharing_mode)
            .queue_family_indices(&queue_family_indicies)
            .pre_transform(support.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());
        let swapchain_loader = ash::khr::swapchain::Device::new(instance, device);
        let swapchain = unsafe {
            swapchain_loader.create_swapchain(&create_info, None)
                .expect("Failed to create swap chain!")
        };
        let swapchain_images = unsafe {
            swapchain_loader.get_swapchain_images(swapchain).unwrap()
        };
        
        println!("Swap chain successfully created! Images: {}", swapchain_images.len());
        
        self.swapchain_loader = Some(swapchain_loader);
        self.swapchain = Some(swapchain);
        self.swapchain_images = swapchain_images;
        self.swapchain_format = format.format;
        self.swapchain_extent = extent;
    }
    fn create_image_views(&mut self) {
        let device = self.device.as_ref().unwrap();
        let image_views: Vec<vk::ImageView> = self.swapchain_images
            .iter()
            .map(|&image| {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(self.swapchain_format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                    
                unsafe {
                    device.create_image_view(&create_info, None)
                        .expect("Failed to create image view!")
                }
            })
            .collect();
            
        println!("Image views successfully created: {}", image_views.len());
        
        self.swapchain_image_views = image_views;
    }
}

fn print_available_extensions(entry: &Entry) {
    let extensions = unsafe {
        entry.enumerate_instance_extension_properties(None).unwrap()
    };

    println!("Available extensions ({}):", extensions.len());
    for ext in &extensions {
        let name = unsafe {
            std::ffi::CStr::from_ptr(ext.extension_name.as_ptr())
        };
        println!("  - {}", name.to_str().unwrap());
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
