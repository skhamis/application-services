/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

import Foundation
import os.log

// SAM: Need to move these cases to the uniffi/udl side 

/// Indicates an error occurred while calling into the places storage layer
 extension PlacesError: LocalizedError {

     // The name is attempting to indicate that we free rustError.message if it
     // existed, and that it's a very bad idea to touch it after you call this
     // function
     static func fromConsuming(_ rustError: PlacesRustError) -> PlacesError? {
         let message = rustError.message

         switch rustError.code {
         case Places_NoError:
            return nil
         case Places_UrlParseError:
            return .UrlParseFailed(message: String(freeingPlacesString: message!))

         case Places_DatabaseBusy:
            return .PlacesConnectionBusy(message: String(freeingPlacesString: message!))

         case Places_DatabaseInterrupted:
            return .OperationInterrupted(message: String(freeingPlacesString: message!))

         case Places_Corrupt:
            return .BookmarksCorruption(message: String(freeingPlacesString: message!))

         case Places_InvalidPlace_InvalidParent:
            return .InvalidParent(message: String(freeingPlacesString: message!))

         case Places_InvalidPlace_UrlTooLong:
            return .UrlTooLong(message: String(freeingPlacesString: message!))

         case Places_InvalidPlace_CannotUpdateRoot:
            return .CannotUpdateRoot(message: String(freeingPlacesString: message!))

//         case Places_Panic:
//            return .InternalPanic(message: String(freeingPlacesString: message!))

         default:
            return .UnexpectedError(message: String(freeingPlacesString: message!))
         }
     }

     @discardableResult
     static func tryUnwrap<T>(_ callback: (UnsafeMutablePointer<PlacesRustError>) throws -> T?) throws -> T? {
         var err = PlacesRustError(code: Places_NoError, message: nil)
         let returnedVal = try callback(&err)
         if let placesErr = PlacesError.fromConsuming(err) {
             throw placesErr
         }
         guard let result = returnedVal else {
             return nil
         }
         return result
     }

     @discardableResult
     static func unwrap<T>(_ callback: (UnsafeMutablePointer<PlacesRustError>) throws -> T?) throws -> T {
         guard let result = try PlacesError.tryUnwrap(callback) else {
             throw ResultError.empty
         }
         return result
     }

     // Same as `tryUnwrap`, but instead of erroring, just logs. Useful for cases like destructors where we
     // cannot throw.
     @discardableResult
     static func unwrapOrLog<T>(_ callback: (UnsafeMutablePointer<PlacesRustError>) throws -> T?) -> T? {
         do {
             let result = try PlacesError.tryUnwrap(callback)
             return result
         } catch let e {
             // Can't log what the error is without jumping through hoops apparently, oh well...
             os_log("Hit places error when throwing is impossible %{public}@", type: .error, "\(e)")
             return nil
         }
     }
 }
