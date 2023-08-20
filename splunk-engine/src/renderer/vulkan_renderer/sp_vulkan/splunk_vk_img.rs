
use ash::{self, vk};
use gpu_allocator::{
    MemoryLocation,
    vulkan::{ 
        Allocation, 
        Allocator, 
        AllocationCreateDesc, 
        AllocationScheme 
    }, 
};

use super::splunk_vk_buffer::create_vk_buffer;

use crate::{vk_check, log_err};

use super::splunk_vk_context::{
    SpVkContext, 
    sp_begin_single_time_vk_command_buffer, 
    sp_end_single_time_vk_command_buffer
};

use std::ffi::c_void;

pub fn create_vk_image(
        device: &ash::Device, allocator: &mut Allocator, label: &str, 
        width: u32, height: u32, 
        format: vk::Format, tiling: vk::ImageTiling, 
        usage: vk::ImageUsageFlags, create_flags: vk::ImageCreateFlags, 
        mip_levels: u32
    ) -> (vk::Image, Allocation)
{
    let create_info = vk::ImageCreateInfo
    {
        s_type: vk::StructureType::IMAGE_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: create_flags,
        image_type: vk::ImageType::TYPE_2D,
        format,
        extent: vk::Extent3D { width, height, depth: 1 },
        mip_levels,
        array_layers: if create_flags == vk::ImageCreateFlags::CUBE_COMPATIBLE { 6 } else { 1 },
        samples: vk::SampleCountFlags::TYPE_1,
        tiling,
        usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        queue_family_index_count: 0,
        p_queue_family_indices: std::ptr::null(),
        initial_layout: vk::ImageLayout::UNDEFINED
    };
    let img = unsafe { vk_check!( device.create_image(&create_info, None) ).unwrap() };
    
    let mem_requirements = unsafe{ device.get_image_memory_requirements(img) };
    
    // let alloc_info = vk::MemoryAllocateInfo
    // {
    //     s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
    //     p_next: std::ptr::null(),
    //     allocation_size: mem_requirements.size,
    //     ..Default::default()
    // };

    // let memory = unsafe{
    //     let mem = vk_check!(device.allocate_memory(&alloc_info, None)).unwrap();
    //     vk_check!(device.bind_image_memory(img, mem, 0)).unwrap();
    //     mem
    // };
    let alloc_info = AllocationCreateDesc
    {
        name: label,
        requirements: mem_requirements,
        location: MemoryLocation::CpuToGpu,
        linear: true,
        allocation_scheme: AllocationScheme::GpuAllocatorManaged
    };
    let allocation = allocator.allocate(&alloc_info).map_err( |e| { log_err!(e); } ).unwrap();

    (img, allocation)
}

pub fn create_vk_image_view(
        device: &ash::Device, image: &vk::Image, 
        format: &vk::Format, aspect_flags: vk::ImageAspectFlags, 
        view_type: vk::ImageViewType, layer_count: u32, mip_levels: u32
    ) -> vk::ImageView
{
    let create_info = vk::ImageViewCreateInfo
    {
        s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: vk::ImageViewCreateFlags::empty(),
        image: *image,
        view_type: view_type,
        format: *format,
        subresource_range: vk::ImageSubresourceRange
        {
            aspect_mask: aspect_flags,
            base_mip_level: 0,
            level_count: mip_levels,
            base_array_layer: 0,
            layer_count: layer_count,
        },
        ..Default::default()
    };

    unsafe { vk_check!( device.create_image_view(&create_info, None) ).unwrap() }
}

pub fn create_vk_sampler(device: &ash::Device) -> vk::Sampler
{
    let create_info = vk::SamplerCreateInfo
    {
        s_type: vk::StructureType::SAMPLER_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: vk::SamplerCreateFlags::empty(),
        mag_filter: vk::Filter::LINEAR,
        min_filter: vk::Filter::LINEAR,
        mipmap_mode: vk::SamplerMipmapMode::LINEAR,
        address_mode_u: vk::SamplerAddressMode::REPEAT,
        address_mode_v: vk::SamplerAddressMode::REPEAT,
        address_mode_w: vk::SamplerAddressMode::REPEAT,
        mip_lod_bias: 0.0,
        anisotropy_enable: vk::TRUE,
        max_anisotropy: 1.0,
        compare_enable: vk::FALSE,
        compare_op: vk::CompareOp::ALWAYS,
        min_lod: 0.0,
        max_lod: 0.0,
        border_color: vk::BorderColor::INT_OPAQUE_BLACK,
        unnormalized_coordinates: vk::FALSE
    };

    unsafe{ vk_check!(device.create_sampler(&create_info, None)).unwrap() }
}

pub fn find_supported_vk_format(instance: &ash::Instance, phys_device: &vk::PhysicalDevice, candidates: &Vec<vk::Format>, tiling: vk::ImageTiling, features: vk::FormatFeatureFlags) -> vk::Format
{
    for format in candidates.iter()
    {
        let props = unsafe { instance.get_physical_device_format_properties(*phys_device, *format) };

        if tiling == vk::ImageTiling::LINEAR && (props.linear_tiling_features & features) == features
        {
            return *format;
        }
        else if tiling == vk::ImageTiling::OPTIMAL && (props.optimal_tiling_features & features) == features
        {
            return *format;
        }
    }

    log_err!("Fn 'find_supported_vk_format()' Failed to find supported format!");
    panic!("Fn 'find_supported_vk_format()' Failed to find supported format!");
}

pub fn has_vk_stencil_component(format: vk::Format) -> bool
{
    format == vk::Format::D32_SFLOAT_S8_UINT || format == vk::Format::D24_UNORM_S8_UINT
}

pub fn transition_vk_image_layout( device: &ash::Device, cmd_buffer: &vk::CommandBuffer, img: vk::Image, format: vk::Format, old_layout: vk::ImageLayout, new_layout: vk::ImageLayout, layer_count: u32, mip_levels: u32)
{
    let mut barrier = vk::ImageMemoryBarrier
    {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
        p_next: std::ptr::null(),
        src_access_mask: vk::AccessFlags::empty(),
        dst_access_mask: vk::AccessFlags::empty(),
        old_layout,
        new_layout,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image: img,
        subresource_range: vk::ImageSubresourceRange
        {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: mip_levels,
            base_array_layer: 0,
            layer_count: layer_count
        },
    };

    let src_stage: vk::PipelineStageFlags;
    let dst_stage: vk::PipelineStageFlags;

    if new_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL ||
        (format == vk::Format::D16_UNORM) ||
        (format == vk::Format::X8_D24_UNORM_PACK32) ||
        (format == vk::Format::D32_SFLOAT) ||
        (format == vk::Format::S8_UINT) ||
        (format == vk::Format::D16_UNORM_S8_UINT) ||
        (format == vk::Format::D32_SFLOAT_S8_UINT)
    {
        barrier.subresource_range.aspect_mask = vk::ImageAspectFlags::DEPTH;
        if has_vk_stencil_component(format)
        {
            barrier.subresource_range.aspect_mask |= vk::ImageAspectFlags::STENCIL;
        }
    } else {
        barrier.subresource_range.aspect_mask |= vk::ImageAspectFlags::COLOR;
    }

    if old_layout == vk::ImageLayout::UNDEFINED && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::empty();
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        src_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
        dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    }
    else if old_layout == vk::ImageLayout::UNDEFINED && new_layout == vk::ImageLayout::GENERAL
    {
        barrier.src_access_mask = vk::AccessFlags::empty();
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        src_stage = vk::PipelineStageFlags::TRANSFER;
        dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    }
    else if old_layout == vk::ImageLayout::UNDEFINED && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::empty();
        barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

        src_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
        dst_stage = vk::PipelineStageFlags::TRANSFER;
    }
    // Convert back from read-only updatable
    else if old_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::SHADER_READ;
        barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

        src_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
        dst_stage = vk::PipelineStageFlags::TRANSFER;
    }
    // Convert from updateable texture to shader read only
    else if old_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        src_stage = vk::PipelineStageFlags::TRANSFER;
        dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    }
    // Convert from depth texture from undefined state to depth stencil buffer
    else if old_layout == vk::ImageLayout::UNDEFINED && new_layout == vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::empty();
        barrier.dst_access_mask = vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;

        src_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
        dst_stage = vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS;
    }
    // Wait for render pass to complete
    else if old_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::empty();
        barrier.dst_access_mask = vk::AccessFlags::empty();

        src_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    }
    // Convert back from read-only to color attachment
    else if old_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL && new_layout == vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::SHADER_READ;
        barrier.dst_access_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;

        src_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
        dst_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
    }
    // Convert from updateable texture to shader read only
    else if old_layout == vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        src_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    }
    // Convert back from read-only to depth attachment
    else if old_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL && new_layout == vk::ImageLayout::DEPTH_READ_ONLY_STENCIL_ATTACHMENT_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::SHADER_READ;
        barrier.dst_access_mask = vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;

        src_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
        dst_stage = vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
    }
    // Convert from updateable depth texture to shader read only
    else if old_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    {
        barrier.src_access_mask = vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        src_stage = vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
        dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    } else {
        src_stage = vk::PipelineStageFlags::NONE;
        dst_stage = vk::PipelineStageFlags::NONE;
    }
    
    unsafe {
        device.cmd_pipeline_barrier(
            *cmd_buffer, 
            src_stage, dst_stage, 
            vk::DependencyFlags::empty(), 
            &[], 
            &[], 
            &[barrier]
        );
    }
}

pub fn copy_vk_buffer_to_img(
        device: &ash::Device, cmd_buffer: &vk::CommandBuffer, 
        buffer: &vk::Buffer, img: &vk::Image, 
        width: u32, height: u32, layer_count: u32
    )
{
    let copy_region = vk::BufferImageCopy
    {
        buffer_offset: 0,
        buffer_row_length: 0,
        buffer_image_height: 0,
        image_subresource: vk::ImageSubresourceLayers
        {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count,
        },
        image_offset: vk::Offset3D{ x: 0, y: 0, z: 0 },
        image_extent: vk::Extent3D{ width, height, depth: 1 }
    };

    unsafe { 
        device.cmd_copy_buffer_to_image(
            *cmd_buffer, 
            *buffer, *img, 
            vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region]
        ); 
    }
}

pub fn find_vk_format_depth_img(instance:&ash::Instance, phys_device: &vk::PhysicalDevice) -> vk::Format
{
    find_supported_vk_format(
        instance, phys_device, 
        &vec![vk::Format::D32_SFLOAT, vk::Format::D32_SFLOAT_S8_UINT, vk::Format::D24_UNORM_S8_UINT ], 
        vk::ImageTiling::OPTIMAL, 
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT
    )
}

pub struct SpVkImage
{
    pub handle: vk::Image,
    pub alloc: Allocation,
    pub view: vk::ImageView,
}

pub fn sp_create_vk_image(vk_ctx: &mut SpVkContext, file_name: &str) -> SpVkImage
{
    let img = image::open(std::path::Path::new(file_name)).map_err( |e| { log_err!(e); } ).unwrap();
    let pixels = img.to_rgba8().into_raw();

    let img_size : vk::DeviceSize = (std::mem::size_of::<u8>() as u32 * img.width() * img.height() * 4) as vk::DeviceSize;
    let staging_buffer: vk::Buffer;
    let staging_allocation: Allocation;

    let label = String::from(format!("staging_allocation: {}", file_name));
    (staging_buffer, staging_allocation) = create_vk_buffer(
        &vk_ctx.device, &mut vk_ctx.allocator, label.as_str(), 
        img_size, vk::BufferUsageFlags::TRANSFER_SRC);

    unsafe
    {
        let p_data = vk_ctx.device.map_memory(staging_allocation.memory(), staging_allocation.offset(), img_size, vk::MemoryMapFlags::empty()).map_err( |e| { log_err!(e); } ).unwrap();
            p_data.copy_from_nonoverlapping(pixels.as_ptr() as *const c_void, pixels.len());    
        vk_ctx.device.unmap_memory(staging_allocation.memory());
    }

    let (handle, alloc) = create_vk_image(
        &vk_ctx.device, &mut vk_ctx.allocator, file_name, 
        img.width(), img.height(), vk::Format::R8G8B8A8_UNORM, 
        vk::ImageTiling::OPTIMAL, vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED, 
        vk::ImageCreateFlags::empty(), 1);
    
    let cmd_buffer = sp_begin_single_time_vk_command_buffer(vk_ctx);

        transition_vk_image_layout(
            &vk_ctx.device, &cmd_buffer, 
            handle, vk::Format::R8G8B8A8_UNORM,
            vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            1, 1);

        copy_vk_buffer_to_img(
            &vk_ctx.device, &cmd_buffer, 
            &staging_buffer, &handle, 
            img.width(), img.height(), 
            1);
        
        transition_vk_image_layout(
            &vk_ctx.device, &cmd_buffer, 
            handle, vk::Format::R8G8B8A8_UNORM, 
            vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, 
            1, 1);

    sp_end_single_time_vk_command_buffer(vk_ctx, cmd_buffer);

    unsafe
    {
        vk_ctx.device.destroy_buffer(staging_buffer, None);
        vk_ctx.allocator.free(staging_allocation).map_err(|e| { log_err!(e); }).unwrap();
    }

    let view = create_vk_image_view(
        &vk_ctx.device, &handle, 
        &vk::Format::R8G8B8A8_UNORM, vk::ImageAspectFlags::COLOR, 
        vk::ImageViewType::TYPE_2D, 
        1, 1);

    SpVkImage { handle, alloc, view }
}

pub fn sp_create_vk_depth_img(instance: &ash::Instance, vk_ctx: &mut SpVkContext, width: u32, height: u32) -> SpVkImage
{
    let format = find_vk_format_depth_img(instance, &vk_ctx.physical_device);
    let (img, alloc) = create_vk_image(
        &vk_ctx.device, &mut vk_ctx.allocator, "depth image",
        width, height, 
        format, vk::ImageTiling::OPTIMAL, 
        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT, 
        vk::ImageCreateFlags::empty(), 1);

    let view = create_vk_image_view(
        &vk_ctx.device, &img, &format, 
        vk::ImageAspectFlags::DEPTH, 
        vk::ImageViewType::TYPE_2D,
        1, 1);

    let cmd_buffer = sp_begin_single_time_vk_command_buffer(vk_ctx);
        transition_vk_image_layout(
            &vk_ctx.device, &cmd_buffer, 
            img, format, 
            vk::ImageLayout::UNDEFINED, 
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, 
            1, 1);
    sp_end_single_time_vk_command_buffer(vk_ctx, cmd_buffer);

    SpVkImage { handle: img, alloc, view }
}

pub fn sp_destroy_vk_img(vk_ctx: &mut SpVkContext, img: SpVkImage)
{
    unsafe
    {
        vk_ctx.device.destroy_image(img.handle, None);
        // device.free_memory(img.memory, None);
        vk_ctx.allocator.free(img.alloc).map_err(|e| { log_err!(e); } ).unwrap();
        vk_ctx.device.destroy_image_view(img.view, None);
    }
}