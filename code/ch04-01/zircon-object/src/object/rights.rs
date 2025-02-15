// ANCHOR: rights
use bitflags::bitflags;

bitflags! {
    /// 句柄权限
    #[derive(Default)]
    pub struct Rights: u32 {
        const DUPLICATE = 1 << 0;
        const TRANSFER = 1 << 1;
        const READ = 1 << 2;
        const WRITE = 1 << 3;
        const EXECUTE = 1 << 4;
        const MAP = 1 << 5;
        const GET_PROPERTY = 1 << 6;
        const SET_PROPERTY = 1 << 7;
        const ENUMERATE = 1 << 8;
        const DESTROY = 1 << 9;
        const SET_POLICY = 1 << 10;
        const GET_POLICY = 1 << 11;
        const SIGNAL = 1 << 12;
        const SIGNAL_PEER = 1 << 13;
        const WAIT = 1 << 14;
        const INSPECT = 1 << 15;
        const MANAGE_JOB = 1 << 16;
        const MANAGE_PROCESS = 1 << 17;
        const MANAGE_THREAD = 1 << 18;
        const APPLY_PROFILE = 1 << 19;
        const SAME_RIGHTS = 1 << 31;

        const BASIC = Self::TRANSFER.bits | Self::DUPLICATE.bits | Self::WAIT.bits | Self::INSPECT.bits;
        const IO = Self::READ.bits | Self::WRITE.bits;

        /// GET_PROPERTY ｜ SET_PROPERTY
        const PROPERTY = Self::GET_PROPERTY.bits | Self::SET_PROPERTY.bits;

        /// GET_POLICY ｜ SET_POLICY
        const POLICY = Self::GET_POLICY.bits | Self::SET_POLICY.bits;

        /// BASIC & !Self::DUPLICATE | IO | SIGNAL | SIGNAL_PEER
        const DEFAULT_CHANNEL = Self::BASIC.bits & !Self::DUPLICATE.bits | Self::IO.bits | Self::SIGNAL.bits | Self::SIGNAL_PEER.bits;

        /// BASIC | IO | PROPERTY | ENUMERATE | DESTROY | SIGNAL | MANAGE_PROCESS | MANAGE_THREAD
        const DEFAULT_PROCESS = Self::BASIC.bits | Self::IO.bits | Self::PROPERTY.bits | Self::ENUMERATE.bits | Self::DESTROY.bits
            | Self::SIGNAL.bits | Self::MANAGE_PROCESS.bits | Self::MANAGE_THREAD.bits;

        /// BASIC | IO | PROPERTY | DESTROY | SIGNAL | MANAGE_THREAD
        const DEFAULT_THREAD = Self::BASIC.bits | Self::IO.bits | Self::PROPERTY.bits | Self::DESTROY.bits | Self::SIGNAL.bits | Self::MANAGE_THREAD.bits;

        /// BASIC | WAIT
        const DEFAULT_VMAR = Self::BASIC.bits & !Self::WAIT.bits;

        /// BASIC | IO | PROPERTY | POLICY | ENUMERATE | DESTROY | SIGNAL | MANAGE_JOB | MANAGE_PROCESS | MANAGE_THREAD
        const DEFAULT_JOB = Self::BASIC.bits | Self::IO.bits | Self::PROPERTY.bits | Self::POLICY.bits | Self::ENUMERATE.bits
            | Self::DESTROY.bits | Self::SIGNAL.bits | Self::MANAGE_JOB.bits | Self::MANAGE_PROCESS.bits | Self::MANAGE_THREAD.bits;

        /// BASIC | IO | PROPERTY | MAP | SIGNAL
        const DEFAULT_VMO = Self::BASIC.bits | Self::IO.bits | Self::PROPERTY.bits | Self::MAP.bits | Self::SIGNAL.bits;

        /// TRANSFER | DUPLICATE | WRITE | INSPECT
        const DEFAULT_RESOURCE = Self::TRANSFER.bits | Self::DUPLICATE.bits | Self::WRITE.bits | Self::INSPECT.bits;
    }
}
// ANCHOR_END: rights
