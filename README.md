# RRadio Web Client

Note that during the refactor of `rradio`, the web client depends on the development branch of `rradio`.

## Building

This app uses [trunk]("https://github.com/thedodd/trunk") as its build system.
To build, run the following from the project root

    trunk build

Then copy the files from the `dist` folder to the static file directory or `rradio`, or specify in `rradio`'s config file
that the static files are found in the `dist` folder of the web app's repository.
