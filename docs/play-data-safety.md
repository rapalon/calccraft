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
| Does your app collect or share any of the required user data types? | **No** | The app has no `INTERNET` permission and no network code or SDK (audit: "Android manifest", "Network"). Everything it stores stays in its private storage; history and memory aren't even stored. Links open in the user's browser only when tapped (but see "The donate link's parameters" below). |
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

### The donate link's parameters

The free build's Donate button opens
`https://rapalon.com/calccraft/donate?...&client_reference_id=cc_android_1-3-2_popup_c3_d30-90`
in the browser. The app itself sends nothing: the browser does, and only after the user taps.
But the app composes the address, and three of its values are derived from the user's use of
the app:

- `utm_medium` (`popup` or `menu`): which reminder the user tapped,
- `c0`–`c10`: how many reminders the user has seen,
- `d0-7` … `d365p`: how long ago the app was first started.

They reach Rapalon Labs' server (through Cloudflare) and Stripe, and Stripe keeps
`client_reference_id` with a donation, next to the donor's name and email. A strict reviewer
can read that as **App activity → App interactions, collected** (optional, for Analytics),
linked to the user when they donate. Answering "No data collected" with these values in place
is defensible (user-initiated, coarse, no identifier) but not safe.

**Proposed minimal change (not made in this task):** in the Play build, make the donate link's
parameters static: `utm_source=calccraft&utm_medium=app&utm_campaign=support&utm_content=android_<version>&client_reference_id=cc_android_<version>`.
Platform and version describe the build, not the user, so the app then sends nothing about
how it was used, and "No data collected" is accurate without caveats. A Cargo feature (for
example `play-store`) or a build-time flag in `donate.rs` can drop the medium, cycle and age
values; `PRIVACY.md`'s Donate section then changes with it. The share link is already static
(platform and version only) and needs no change.

Until Konst decides, `PRIVACY.md` describes the current parameters exactly, so the policy and
the form don't contradict each other either way: with "No data collected", the policy explains
that only coarse, non-identifying values go out when the user taps Donate.

## App content declarations

| Declaration | Answer | Why |
| --- | --- | --- |
| Privacy policy | `https://rapalon.com/calccraft/privacy` | `PRIVACY.md`. |
| Ads | **No, my app does not contain ads** | No ad SDK; the support reminder is the app's own request for donations, not an ad, and links only to the developer's page. |
| App access | **All functionality is available without special access** | No login, account, subscription or region lock. |
| Content rating | Answer the IARC questionnaire with "No" to everything (no violence, sexuality, language, controlled substances, gambling, user interaction, sharing of location or personal information, or digital purchases inside the app) | A calculator. Donations happen on a web page, not as an in-app purchase. Expected rating: Everyone / PEGI 3. |
| Target audience and content | **13–15, 16–17 and 18+** (not under 13) | Recommended by the task: an age group under 13 brings the Families policy (teacher-approved program rules, stricter link and donation rules for children). The app collects no data, so choosing 13+ costs nothing; the policy says it isn't directed to children under 13. "Appeal to children": No. |
| News app | No | |
| COVID-19 contact tracing and status | No | |
| Data safety | As above | |
| Government apps | No | |
| Financial features | None | Donations are voluntary payments on Stripe's page; the app offers no financial service. |
| Health apps | No | |
| Advertising ID | **No**, the app doesn't use the advertising ID | No Google Play services, no `AD_ID` permission (no permissions at all). |

## Open decisions for Konst

1. **Postal address.** The policy names Rapalon Labs, Inc. (a Delaware corporation) and
   privacy@rapalon.com. No postal address is published anywhere in the repository, so none
   is in the policy. Google doesn't require one in the policy, but the Play developer account
   shows one for organizations; add it to `PRIVACY.md` if it should be there too.
2. **Donate link parameters in the Play build:** keep them (and accept the "App activity"
   risk above) or make them static for Play (recommended).
3. **`allowBackup`:** keep it (settings come back on a new phone, and the policy explains it) or
   set it to `false`. Keeping it is fine for Data safety.
4. **rapalon.com:** `PRIVACY.md` assumes the website itself adds no analytics or cookies
   beyond Cloudflare's delivery and the aggregate UTM counts. If the website uses an analytics
   tool, the website task should name it in the published page.
