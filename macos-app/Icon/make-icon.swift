// Draws the app icon — a monitor in the style of the menu bar icon — as an
// .iconset folder for `iconutil`. Drawn as vectors at every size, so it stays
// sharp from 16 px to 1024 px.
//
// Usage: swift make-icon.swift <output.iconset>
// (Not an SF Symbol: Apple's license doesn't allow those in app icons.)

import AppKit

let output = URL(filePath: CommandLine.arguments[1])
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)

/// Draws the icon on a 1024-point canvas, scaled to `pixels`.
func render(pixels: Int) -> Data {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let scale = CGFloat(pixels) / 1024
    NSGraphicsContext.current!.cgContext.scaleBy(x: scale, y: scale)

    // Background tile: the standard macOS icon grid (824 pt tile, 100 pt margin).
    let tile = NSRect(x: 100, y: 100, width: 824, height: 824)
    let tilePath = NSBezierPath(roundedRect: tile, xRadius: 185, yRadius: 185)
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowColor = .black.withAlphaComponent(0.3)
    shadow.shadowOffset = NSSize(width: 0, height: -12)
    shadow.shadowBlurRadius = 24
    shadow.set()
    NSColor.black.setFill()
    tilePath.fill()
    NSGraphicsContext.restoreGraphicsState()
    NSGradient(
        starting: NSColor(srgbRed: 0.24, green: 0.45, blue: 0.95, alpha: 1),
        ending: NSColor(srgbRed: 0.13, green: 0.20, blue: 0.55, alpha: 1)
    )!.draw(in: tilePath, angle: -90)

    // The monitor: an outlined screen on a short neck and a flat base.
    let white = NSColor.white
    let line: CGFloat = 46
    let screen = NSRect(x: 232, y: 392, width: 560, height: 360)
    let screenPath = NSBezierPath(roundedRect: screen, xRadius: 48, yRadius: 48)
    NSGradient(
        starting: white.withAlphaComponent(0.28), ending: white.withAlphaComponent(0.08)
    )!.draw(in: screenPath, angle: -90)
    white.setStroke()
    screenPath.lineWidth = line
    screenPath.stroke()

    white.setFill()
    NSBezierPath(rect: NSRect(x: 482, y: 300, width: 60, height: 92 - line / 2)).fill()
    NSBezierPath(
        roundedRect: NSRect(x: 362, y: 272, width: 300, height: line),
        xRadius: line / 2, yRadius: line / 2
    ).fill()

    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

for points in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let name = scale == 1 ? "icon_\(points)x\(points).png" : "icon_\(points)x\(points)@2x.png"
        try render(pixels: points * scale).write(to: output.appending(path: name))
    }
}
