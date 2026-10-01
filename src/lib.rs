#![no_std]
#![feature(impl_trait_in_assoc_type)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

extern crate alloc;

pub mod common;

pub mod config;
pub mod display;

pub mod ns_api;

pub mod web_server;
pub mod wifi;