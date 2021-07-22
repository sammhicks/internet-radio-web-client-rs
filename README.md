# RRadio Web Client

## Building

This app uses [trunk]("https://github.com/thedodd/trunk") as its build system.

## Pages

### Podcasts Interface - `/?podcasts`

Enter a url to an RSS feed, and click on "Add Podcast" to add the podcast to the list.

Then select a podcast to see its entries, and click on "Play" next to an item to play it.

### Debug Interface - `/`

Used to display debug information about the current state

## Configuration

By default, the app connects to the same host as the server hosting the web app.
To override this, set the "RRADIO_SERVER" value in the Local Storage area of Web Storage in your browser.
