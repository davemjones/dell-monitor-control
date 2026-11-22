import CoreGraphics

/// Where a control's panel goes: the top-right corner of the screen's visible
/// area (below the menu bar), inset slightly, where Control Center and most
/// menu bar items are. The app can't tell where the control itself was
/// clicked, so the panel always opens here.
public func panelOrigin(size: CGSize, visibleFrame: CGRect, inset: CGFloat = 8) -> CGPoint {
    CGPoint(
        x: max(visibleFrame.minX, visibleFrame.maxX - size.width - inset),
        y: max(visibleFrame.minY, visibleFrame.maxY - size.height - inset)
    )
}
