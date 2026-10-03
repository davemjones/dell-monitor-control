extension InputEntry {
    /// How the panel names this input; dellctl's name if it isn't a known one.
    public var label: String {
        switch name {
        case "thunderbolt": "Thunderbolt / USB-C"
        case "dp": "DisplayPort"
        case "dp2": "DisplayPort 2"
        case "hdmi": "HDMI"
        case "hdmi2": "HDMI 2"
        case "vga": "VGA"
        case "dvi": "DVI"
        default: name
        }
    }
}
