pub mod result_set;

struct CapabilityFlag {
    pub bitmask: u32,
}

#[repr(u32)]
pub enum CapabilityFlags {
    ClientLongPassword = 0x01,
    ClientPluginAuth = 0x1 << 19,
}
