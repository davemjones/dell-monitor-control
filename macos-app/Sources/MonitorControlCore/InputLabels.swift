import Foundation

extension InputEntry {
    /// How the panel names this input: the user's name for it, else `builtInLabel`.
    public var displayName: String {
        label ?? builtInLabel
    }

    /// The panel's name for this kind of input; monitorctl's name if it isn't a known one.
    public var builtInLabel: String {
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

    /// The custom name typed in the panel means: nil (no custom name) when
    /// it's empty or the built-in name.
    public func newLabel(from text: String) -> String? {
        let name = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return name.isEmpty || name == builtInLabel ? nil : name
    }

    /// The `monitorctl` arguments that give this input `label`, or clear it.
    public func renameArguments(_ label: String?) -> [String] {
        ["input", "name", code] + (label.map { [$0] } ?? ["--clear"]) + ["--json"]
    }
}
