# CalcCraft Privacy Policy

**Effective date:** November 19, 2026

This Privacy Policy explains how the CalcCraft app handles your information. It covers
CalcCraft on every platform we publish it for: Android (the free CalcCraft and the paid
CalcCraft Pro on Google Play, F-Droid, GitHub Releases and other stores we list), Windows,
macOS, Linux and the web version in your browser. Our
[Terms of Service](https://rapalon.com/calccraft/terms/) apply to the app as well.

## Who we are

CalcCraft is developed and published by **Rapalon Labs, Inc.**, a Delaware corporation
("Rapalon Labs", "we", "us"). Website: <https://rapalon.com>.

Questions or requests about privacy: **[privacy@rapalon.com](mailto:privacy@rapalon.com)**.

## Summary

- CalcCraft does not collect, store on our servers, or share personal data.
- There are no accounts, ads, analytics, tracking or crash reporting in the app, and no
  third-party SDKs that collect data.
- The app makes no network requests of its own. It works fully offline.
- On Android its only permission is vibration, used only to give key presses a tap of the
  vibration motor (Android grants it without asking). It has no internet access, location,
  contacts, camera, microphone, storage or phone permission.
- Your calculations, history and memory never leave your device. They aren't even saved when
  you close the app.
- Something leaves your device only when you tap a link (such as Donate) or share something
  yourself, and then only what is described below.
- The Google Play versions have no Donate link and put no parameters into any link they open
  or share (see "Google Play versions").
- If you buy CalcCraft Pro, Google Play handles the payment. We never see your payment details.

## What the app uses on your device

CalcCraft reads a few settings of your device so that it looks and works the way you expect.
This information is used on your device only and is never sent anywhere:

- your language and regional formats (number, date and list formats, first day of the week),
  and your country or region, to choose the default units of the converters;
- the light or dark theme of your system;
- on Android, the size of the system bars, so the app doesn't draw under them, and whether
  touch feedback is turned on in Android's settings, so that keys vibrate only when it is;
- the clipboard, only when you copy (`Ctrl+C`, Copy) or paste (`Ctrl+V`, Paste) yourself. In
  the web version, opening a menu that has Paste starts reading the clipboard's text (your
  browser may ask you first), so that it's ready if you pick Paste; it isn't used otherwise.

## What is stored on your device, and how to delete it

CalcCraft saves a small settings file on your device:

- your choices: app theme (and colour theme, in versions that offer a choice of them),
  expression size, language, regional format, the screen orientation and the key vibration
  (Android), the last calculator mode, the last converter category and units, the graph
  theme, the size of the keep-on-top window, and the app's own interface state (such as
  scroll positions);
- the counters behind the occasional "Please support us" reminder (kept by every version,
  including those that never show the reminder):
  how many times the app was started, the number of calculations and of days with a
  calculation, day numbers (such as the day of the first start and when the next reminder may
  show), how many reminders were shown, whether you tapped Donate, Tell a friend or
  "I already supported". These are counters and dates only. They don't identify you or your
  device, and they never leave your device except as the coarse values described under
  "Donate" below, which the Google Play versions don't send at all.

Your calculations, history, memory, equations and dates are kept only while the app is open.

We have no copy of this file and cannot see it. You can delete it at any time:

- **Android:** it is in the app's private storage. Go to Settings → Apps → CalcCraft →
  Storage → Clear storage, or uninstall the app.
- **Windows:** delete `%APPDATA%\CalcCraft\data\app.ron`, or the whole `CalcCraft` folder.
- **macOS:** delete `~/Library/Application Support/CalcCraft/app.ron`, or its folder.
- **Linux:** delete `~/.local/share/calccraft/app.ron` (or the same file under
  `$XDG_DATA_HOME`), or its folder.
- **Web:** it is in your browser's local storage for the site. Clear the site's data in your
  browser's settings.

On Android, if you have turned on backup for your Google account, Android may include this
settings file in your device backup. That backup is made and kept by Google under your
account, and we have no access to it. Clearing the app's storage removes the file from the
device; you can manage backups in your Android or Google account settings.

## What leaves your device when you tap a link

The app opens links in your browser (or your browser's tab) only when you tap them. Your
browser then loads the page like any other: the website and the services that deliver it see
your IP address and your browser's usual information (see "Third-party services").

These are the links in the app, where they are, and what they add to the address:

- **Rapalon Labs** (About card, support window) opens [rapalon.com](https://rapalon.com).
  Nothing is added.
- **Privacy policy** (Settings) opens this policy at
  [rapalon.com/calccraft/privacy](https://rapalon.com/calccraft/privacy). Nothing is added.
- **Send feedback** (Settings) opens
  [CalcCraft's issues on GitHub](https://github.com/rapalon/calccraft/issues). Nothing is
  added.
- **GitHub** (Settings) opens
  [CalcCraft's repository on GitHub](https://github.com/rapalon/calccraft). Nothing is added.
- **Donate** (support window; GitHub, F-Droid, desktop and web versions only) opens
  [rapalon.com/calccraft/donate](https://rapalon.com/calccraft/donate), which forwards you to
  Stripe's payment page. The address carries the parameters described below.
- **Get CalcCraft Pro** (support window of the free Google Play app) opens CalcCraft Pro's page
  in the Google Play Store app, or on [play.google.com](https://play.google.com). Nothing is
  added but that app's ID.

**Donate.** Only the GitHub, F-Droid, desktop and web versions have it. The donate address
carries these parameters:

- `utm_source=calccraft` and `utm_campaign=support`: always the same;
- `utm_medium`: `popup` if the reminder opened by itself, `menu` if you opened it from the menu;
- `utm_content`: the platform (`android`, `win`, `mac`, `linux` or `web`) and the app version;
- `client_reference_id`: the same platform, version and `popup`/`menu`, plus how many
  reminders you have seen (`c0` to `c10`, where `c10` means ten or more) and a coarse range of
  days since you first started the app (`d0-7`, `d7-30`, `d30-90`, `d90-365` or `d365p`).

For example, version 1.3.2 on Android, opened by the reminder for the third time, 30 to 90
days after the first start, adds `utm_medium=popup`, `utm_content=android_1-3-2`, and `client_reference_id` with the
value `cc_android_1-3-2_popup_c3_d30-90`.

These values contain no name, no identifier of you or your device, and nothing you calculated.
We use them only in aggregate, to see which reminders and platforms lead to donations. Stripe
keeps `client_reference_id` with a donation made from that page.

**Tell a friend.** This puts a short message with the link `https://rapalon.com/calccraft` on
your clipboard, with the parameters `utm_source=calccraft`, `utm_medium=share` and
`utm_content=<platform>_<version>` added, or opens your system's share sheet, where you choose the app and the person
to send it to. Nothing is sent until you do so, and then it goes through the app you chose.
The Google Play versions share the plain link `https://rapalon.com/calccraft`, without
parameters.

**Sharing a graph.** In the Graphing mode, Share opens your system's share sheet (on Windows
and Android) or downloads a picture (in the browser), and Copy puts the graph on your
clipboard. The picture and your equations go only to the app or the place you choose. On
Android the picture is first saved as one file in the app's private cache, where only the app
you choose may read it; each share replaces it, so there is never more than one. It isn't put
in your gallery or shared storage. Android may delete it at any time, and Clear storage or
Clear cache in the app's settings deletes it.

## Google Play versions

CalcCraft on Google Play comes as a free app and a paid app, CalcCraft Pro. Neither has a Donate
link or sends the donate parameters above, and no link they open or share carries parameters:

- The free app's occasional reminder offers "Get CalcCraft Pro", which opens CalcCraft Pro's
  Google Play page with nothing added but that app's ID. (Free versions built before CalcCraft
  Pro was published show no reminder at all.)
- CalcCraft Pro shows no reminder.
- Buying CalcCraft Pro happens entirely in Google Play, under Google's terms and privacy
  policy. Google Play takes the payment, handles refunds and taxes, and keeps your payment
  details. The app itself handles no purchase data: it has no in-app purchases and no
  purchase or licence check of its own. We receive from Google only the order and sales
  reports it gives developers, never your card or other payment details. We use them only to
  keep the records the law requires and to answer you if you contact us about an order.

## Donations

This section applies to the GitHub, F-Droid, desktop and web versions only. Donations are
optional and are made on Stripe's own payment page, not in the app. Stripe
collects the payment details you enter there under its own privacy policy. As the recipient,
Rapalon Labs receives from Stripe what Stripe provides about a payment (such as the amount, the
date, your name, email address and country, and the `client_reference_id` above), and uses it
only to process and keep records of the donation as the law requires. We don't receive your
full card number.

## Third-party services

We don't share any data with third parties, because the app collects none. Some services that
distribute CalcCraft or deliver its web pages process data under their own policies:

- **Google Play** (Android): installs, purchases, reviews, and, if you allow it on your device,
  crash and "app not responding" reports that Android sends to Google (Android vitals), which we
  may see in aggregate. [Google Privacy Policy](https://policies.google.com/privacy)
- **GitHub** (GitHub Releases downloads, the source code and issues, and GitHub Pages, which
  serves the web version): GitHub's servers log requests, including IP addresses.
  [GitHub General Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement)
- **F-Droid**, if you install CalcCraft from it. [F-Droid](https://f-droid.org)
- **Cloudflare**, which delivers rapalon.com, including the donate and share pages.
  [Cloudflare Privacy Policy](https://www.cloudflare.com/privacypolicy/)
- **Stripe**, which handles donations (not in the Google Play versions).
  [Stripe Privacy Policy](https://stripe.com/privacy)

## Children

CalcCraft is a general-audience utility. It is not directed at children under 13, and on
Google Play its target audience is ages 13–15, 16–17 and 18 and over. Google Play notes that in
some countries users in these age groups count as children, so the rules that protect
children, such as Google Play's Families policy, the U.S. Children's Online Privacy Protection
Act (COPPA) and the EU's General Data Protection Regulation (GDPR), apply whenever a child uses
the app.

CalcCraft collects no personal data from anyone, children included. It has no ads, accounts,
analytics or third-party SDKs, so it has no need to ask anyone's age, and it doesn't. Users
below the age of digital consent in their country (up to 16 in parts of the EU) are treated
exactly like everyone else: nothing is collected from them.

## Security

The app has no servers or accounts to protect, because it sends nothing to us. The settings
file stays in the app's private storage on Android and in your user profile on desktops. All
the links the app opens use HTTPS, so what your browser sends is encrypted in transit.

## Changes to this policy

If CalcCraft starts handling data differently, we will update this policy before that version
is released, and change the effective date above. The source of this policy is
[`PRIVACY.md`](https://github.com/rapalon/calccraft/blob/main/PRIVACY.md) in the app's
repository, whose history keeps every dated revision.

## Contact

Rapalon Labs, Inc. — [privacy@rapalon.com](mailto:privacy@rapalon.com)
