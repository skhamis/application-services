

import Foundation
#if canImport(MozillaRustComponents)
    import MozillaRustComponents
#endif


class DeviceCommands {

    private var persistCallback: PersistCallback?
    private var fxAccount: FirefoxAccount

    init(fxAccount: FirefoxAccount) {
        self.fxAccount = fxAccount
    }


    public func closeRemoteTabs(targetDeviceId: String, title: String, url: String) throws {
        return try notifyAuthErrors {
            // Tell tabs to do db stuff
            // Tell FxA to send notification
        }
    }
}