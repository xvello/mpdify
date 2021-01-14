# mpdify &nbsp; &nbsp; [![CI status](https://github.com/xvello/mpdify/workflows/CI/badge.svg?branch=master)](https://github.com/xvello/mpdify/actions) [![dependency status](https://deps.rs/repo/github/xvello/mpdify/status.svg)](https://deps.rs/repo/github/xvello/mpdify)
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fxvello%2Fmpdify.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2Fxvello%2Fmpdify?ref=badge_shield)

An experimental frontend to the Spotify public API, exposing a virtual [MPD server](https://www.musicpd.org/doc/html/protocol.html) and HTTP endpoints.

## Design goals

After migrating my home music setup from MPD to Spotify, I noticed the following regressions:
  - The android app is a lot heavier than [MPDroid](https://github.com/abarisain/dmix/blob/master/README.md) that I used on my old phone
  - A Spotify player is linked to a single account, my parter cannot easily control playback of the instance running on my domotic server
  - Integrations with domotics systems are few and partial

The goal of this project is to solve these regressions by allowing control of a Spotify instance by [existing MPD clients](https://www.musicpd.org/clients/).
I will test it heavily with [Cantata](https://github.com/CDrummond/cantata/blob/master/README.md) and [MPDroid](https://github.com/abarisain/dmix/blob/master/README.md)
but please reach out if you notice strange behaviours with other clients.


## License
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fxvello%2Fmpdify.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2Fxvello%2Fmpdify?ref=badge_large)