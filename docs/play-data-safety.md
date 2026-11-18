# Google Play: Data safety and App content answers

The answers for Play Console, derived from the code audit in [`privacy-audit.md`](privacy-audit.md)
(version 1.3.2, `main` at `a7310d2`). The privacy policy they must agree with is
[`PRIVACY.md`](../PRIVACY.md), published at <https://rapalon.com/calccraft/privacy>.

## Data safety form

Google's definitions: data is **collected** when the app transmits it off the device, and
**shared** when it's transferred to a third party. Data processed only on the device isn't
collected and isn't declared.

| Question | Answer | Why |
| --- | --- | --- |
| Does your app collect or share any of the required user data types? | **No** | The app has no `INTERNET` permission (its only permission is `VIBRATE`, for the keys' vibration) and no network code or SDK (audit: "Android manifest", "Network"). Everything it stores stays in its private storage; history and memory aren't even stored. Links open in the user's browser only when tapped, and the Play builds' links carry no parameters (see "Link parameters in the Play builds" below). |
| Is all of the user data collected by your app encrypted in transit? | Not asked when the answer above is No. If it ever is: **Yes** | Every link the app opens is HTTPS. |
| Do you provide a way for users to request that their data is deleted? | Not asked when nothing is collected. If it ever is: **Yes**, by email to privacy@rapalon.com, and locally by clearing the app's storage | `PRIVACY.md`, "What is stored on your device, and how to delete it". |
| Privacy policy URL | `https://rapalon.com/calccraft/privacy` | Must be live, public HTML, not geofenced, before the release is submitted. |

Notes for the form's fine print:

- **Device or other IDs:** none. The app reads no advertising ID, Android ID, IMEI or account.
- **App activity / App info and performance:** nothing sent by the app. Crash and ANR data
  in Android vitals is collected by Google's system, not by the app, and isn't declared by the
  developer.
- **Android backup** (`allowBackup="true"`): the settings file may go to the user's own Google
  backup, which Google handles under the user's account. It isn't collection by the developer,
  and `PRIVACY.md` mentions it.
- **Sharing a graph** (1.4.1): the user taps Share and picks an app in Android's share sheet;
  only then does that app read the picture, from one file in the app's private cache that
  every share replaces (a non-exported `ContentProvider` of the app's own, audit: "Android
  manifest"). A transfer the user starts in the system's share sheet isn't collection or
  sharing by the developer, and nothing goes to the developer: the answers stay **No**.

### Link parameters in the Play builds

The GitHub, F-Droid, desktop and web builds' Donate button opens
`https://rapalon.com/calccraft/donate?...&client_reference_id=cc_android_1-3-2_popup_c3_d30-90`,
whose `utm_medium`, cycle count (`c0`–`c10`) and age bucket (`d0-7` … `d365p`) come from the
user's use of the app. A strict reviewer could read them as **App activity → App interactions,
collected**.

The Google Play builds don't have that link at all (Konst's decision of 2026-11-17: Play's
Payments policy forbids other payment methods). They are separate build channels, Cargo
features (docs/support-prompt.md, section 8; android/README.md, "Channels"):

- `play-free`, the free app: no support prompt until the paid app exists; then "Get CalcCraft
  Pro" opens `market://details?id=com.rapalon.calccraft.pro` (or the same listing on
  play.google.com), with nothing but the ID.
- `play-paid`, CalcCraft Pro: no support prompt.
- Both share `https://rapalon.com/calccraft` without parameters.

`build-apk.ps1` fails a Play build whose library contains `rapalon.com/calccraft/donate` or
`stripe.com`, and CI builds every Play variant. So the Play apps put nothing derived from their
use into any link, and "No data collected" holds without caveats. The paid app answers the form
the same way: it has the same code, minus the prompt.

## App content declarations

| Declaration | Answer | Why |
| --- | --- | --- |
| Privacy policy | `https://rapalon.com/calccraft/privacy` | `PRIVACY.md`. |
| Ads | **No, my app does not contain ads** | No ad SDK. In the free Play app the support reminder (once CalcCraft Pro exists) is the developer's own suggestion of its paid edition on Google Play, not a third-party ad; before that, and in the paid app, there is none. |
| App access | **All functionality is available without special access** | No login, account, subscription or region lock. |
| Content rating | Answer the IARC questionnaire with "No" to everything (no violence, sexuality, language, controlled substances, gambling, user interaction, sharing of location or personal information, or digital purchases inside the app) | A calculator. The Play builds have no donations and no in-app purchases; CalcCraft Pro is a separate paid app. Expected rating: Everyone / PEGI 3. |
| Target audience and content | **13–15, 16–17 and 18+** (not under 13) | Recommended by the task: an age group under 13 brings the Families policy (teacher-approved program rules, stricter link and donation rules for children). The app collects no data, so choosing 13+ costs nothing; the policy says it isn't directed to children under 13. "Appeal to children": No. |
| News app | No | |
| COVID-19 contact tracing and status | No | |
| Data safety | As above | |
| Government apps | No | |
| Financial features | None | The Play builds have no payments at all (no Donate link); the app offers no financial service. |
| Health apps | No | |
| Advertising ID | **No**, the app doesn't use the advertising ID | No Google Play services, no `AD_ID` permission (the only permission is `VIBRATE`, for the keys' vibration). |

## Open decisions for Konst

1. **Postal address.** The policy names Rapalon Labs, Inc. (a Delaware corporation) and
   privacy@rapalon.com. No postal address is published anywhere in the repository, so none
   is in the policy. Google doesn't require one in the policy, but the Play developer account
   shows one for organizations; add it to `PRIVACY.md` if it should be there too.
2. ~~Donate link parameters in the Play build~~: resolved on 2026-11-17. The Play builds have
   no Donate link and no link parameters (above).
3. **`allowBackup`:** keep it (settings come back on a new phone, and the policy explains it) or
   set it to `false`. Keeping it is fine for Data safety.
4. **rapalon.com:** `PRIVACY.md` assumes the website itself adds no analytics or cookies
   beyond Cloudflare's delivery and the aggregate UTM counts. If the website uses an analytics
   tool, the website task should name it in the published page.
