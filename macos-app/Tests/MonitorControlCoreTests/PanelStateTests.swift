import Foundation
import Testing

@testable import MonitorControlCore

private func finished(_ code: Int32, stdout: String = "", stderr: String = "") -> RunResult {
    .finished(exitCode: code, stdout: Data(stdout.utf8), stderr: Data(stderr.utf8))
}

private let u3425we = """
    {
      "monitor": 1,
      "name": "DELL U3425WE",
      "input": { "name": "thunderbolt", "code": "0x19" },
      "inputs": [
        { "name": "dp", "code": "0x0f", "active": false },
        { "name": "hdmi", "code": "0x11", "active": false },
        { "name": "thunderbolt", "code": "0x19", "active": true }
      ],
      "brightness": 78,
      "volume": 88,
      "speakers": null
    }
    """

@Test func readsStatus() throws {
    let state = PanelState(statusResult: finished(0, stdout: u3425we))
    guard case .ready(let status) = state else {
        Issue.record("expected ready, got \(state)")
        return
    }
    #expect(status.name == "DELL U3425WE")
    #expect(status.brightness == 78)
    #expect(status.inputs?.map(\.name) == ["dp", "hdmi", "thunderbolt"])
    #expect(status.inputs?.filter(\.active).map(\.name) == ["thunderbolt"])
}

@Test func missingSectionsStayReady() {
    let json = #"{"monitor":1,"name":"DELL P2419H","input":null,"inputs":null,"brightness":40}"#
    guard case .ready(let status) = PanelState(statusResult: finished(0, stdout: json)) else {
        Issue.record("expected ready")
        return
    }
    #expect(status.inputs == nil)
    #expect(status.brightness == 40)
}

@Test func nothingReadableMeansUnreachable() {
    let json = #"{"monitor":1,"name":"DELL U3425WE","input":null,"inputs":null,"brightness":null}"#
    #expect(
        PanelState(statusResult: finished(0, stdout: json))
            == .problem(.unreachable(name: "DELL U3425WE")))
}

@Test func severalMonitorsAreNotSupported() {
    #expect(
        PanelState(statusResult: finished(0, stdout: "[\(u3425we), \(u3425we)]"))
            == .problem(.multipleMonitors))
    let err = #"{"error":{"kind":"multiple_monitors","message":"several monitors"}}"#
    #expect(PanelState(statusResult: finished(1, stderr: err)) == .problem(.multipleMonitors))
}

@Test func errorKinds() {
    let none = #"{"error":{"kind":"no_monitor","message":"no DDC/CI-capable monitors found"}}"#
    #expect(PanelState(statusResult: finished(1, stderr: none)) == .problem(.noMonitor))
    let other = #"{"error":{"kind":"failed","message":"couldn't read brightness"}}"#
    #expect(
        PanelState(statusResult: finished(1, stderr: other))
            == .problem(.failed(message: "Couldn't read brightness")))
    #expect(
        PanelState(statusResult: finished(1, stderr: "error: boom\n"))
            == .problem(.failed(message: "Error: boom")))
}

@Test func launchAndTimeout() {
    #expect(PanelState(statusResult: .launchFailed) == .problem(.cantRunDellctl))
    #expect(PanelState(statusResult: .timedOut) == .problem(.timedOut))
}

@Test func runsAProcess() async {
    let echo = Dellctl(executable: URL(filePath: "/bin/echo"))
    #expect(await echo.run(["hi"]) == finished(0, stdout: "hi\n"))
    let missing = Dellctl(executable: URL(filePath: "/nonexistent/dellctl"))
    #expect(await missing.run([]) == .launchFailed)
    let slow = Dellctl(executable: URL(filePath: "/bin/sleep"))
    #expect(await slow.run(["5"], timeout: 0.2) == .timedOut)
}
