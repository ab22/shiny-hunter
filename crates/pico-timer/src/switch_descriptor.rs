// USB HID report descriptor — matches usb_descriptors.c exactly
// (HORI Pokken Controller, VID=0x0F0D PID=0x0092).
//
// Report layout (8 bytes total):
//   [0-1] buttons: 16 button bits (no padding — fills both bytes completely)
//   [2]   hat:     4-bit hat switch (0=Up,2=Right,4=Down,6=Left,8=Neutral) + 4 pad bits
//   [3]   x:       left stick X  (0-255, 128=center)
//   [4]   y:       left stick Y  (0-255, 128=center)
//   [5]   z:       right stick X (0-255, 128=center)
//   [6]   rz:      right stick Y (0-255, 128=center)
//   [7]   vendor:  reserved, always 0
pub const DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x09, 0x05,        // Usage (Joystick)
    0xA1, 0x01,        // Collection (Application)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x35, 0x00,        //   Physical Minimum (0)
    0x45, 0x01,        //   Physical Maximum (1)
    0x75, 0x01,        //   Report Size (1)
    0x95, 0x10,        //   Report Count (16) — 16 bits, fills 2 bytes with no padding
    0x05, 0x09,        //   Usage Page (Button)
    0x19, 0x01,        //   Usage Minimum (Button 1)
    0x29, 0x10,        //   Usage Maximum (Button 16)
    0x81, 0x02,        //   Input (Data, Variable, Absolute)
    0x05, 0x01,        //   Usage Page (Generic Desktop)
    0x25, 0x07,        //   Logical Maximum (7)
    0x46, 0x3B, 0x01,  //   Physical Maximum (315)
    0x75, 0x04,        //   Report Size (4)
    0x95, 0x01,        //   Report Count (1)
    0x65, 0x14,        //   Unit (English Rotation)
    0x09, 0x39,        //   Usage (Hat switch)
    0x81, 0x42,        //   Input (Data, Variable, Absolute, Null state)
    0x65, 0x00,        //   Unit (None)
    0x95, 0x01,        //   Report Count (1) — 4 padding bits
    0x81, 0x01,        //   Input (Constant)
    0x26, 0xFF, 0x00,  //   Logical Maximum (255)
    0x46, 0xFF, 0x00,  //   Physical Maximum (255)
    0x09, 0x30,        //   Usage (X)
    0x09, 0x31,        //   Usage (Y)
    0x09, 0x32,        //   Usage (Z)
    0x09, 0x35,        //   Usage (Rz)
    0x75, 0x08,        //   Report Size (8)
    0x95, 0x04,        //   Report Count (4)
    0x81, 0x02,        //   Input (Data, Variable, Absolute)
    0x06, 0x00, 0xFF,  //   Usage Page (Vendor Defined 0xFF00)
    0x09, 0x20,        //   Usage (0x20)
    0x95, 0x01,        //   Report Count (1)
    0x81, 0x02,        //   Input (Data, Variable, Absolute)
    // Output report (8 bytes) required by Switch for this device class.
    0x09, 0x21,        //   Usage (Vendor)
    0x95, 0x08,        //   Report Count (8)
    0x91, 0x02,        //   Output (Data, Variable, Absolute)
    0xC0,              // End Collection
];

#[repr(C, packed)]
pub struct SwitchGamepadDescriptor {
    pub buttons: u16,
    pub hat: u8,
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub rz: u8,
    pub vendor: u8,
}

impl SwitchGamepadDescriptor {
    pub fn neutral() -> Self {
        Self {
            buttons: 0,
            hat: SwitchHatValues::Neutral as u8,
            x: 128,
            y: 128,
            z: 128,
            rz: 128,
            vendor: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8; 8] {
        // Safety: repr(C, packed) guarantees no padding and exact 8-byte layout.
        unsafe { &*(self as *const Self as *const [u8; 8]) }
    }
}

// Must be repr(u16) — values BtnMinus and above exceed u8::MAX.
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum SwitchButton {
    BtnY     = 1 << 0,
    BtnB     = 1 << 1,
    BtnA     = 1 << 2,
    BtnX     = 1 << 3,
    BtnL     = 1 << 4,
    BtnR     = 1 << 5,
    BtnZL    = 1 << 6,
    BtnZR    = 1 << 7,
    BtnMinus = 1 << 8,
    BtnPlus  = 1 << 9,
    BtnLS    = 1 << 10,
    BtnRS    = 1 << 11,
    BtnHome  = 1 << 12,
    BtnCap   = 1 << 13,
}

#[repr(u8)]
pub enum SwitchHatValues {
    Up      = 0x00,
    Right   = 0x02,
    Down    = 0x04,
    Left    = 0x06,
    Neutral = 0x08,
}
