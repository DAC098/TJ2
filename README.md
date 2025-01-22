# Thoughts Journal 2 (TJ2)

# Work In Progress

This is not ready for production use and there is still a lot of work needed to
be done.

## Setup / Build

Work will needs to be done do make the setup process easier to manage and not
have it be as manual.

### Requirements

The current versions of software are required in order to run / build the server
and frontend (previous or newer versions may work but have not been tested).

 - Rust (1.81.0)
 - PostgreSQL (17)
 - NodeJS (22.9.0)

all of the library dependencies will be handle by either `cargo` or `npm` when
building the server or frontend.

the server will require a private key and certificate as development has been
done using HTTPS and sessions may not work properly in the browser without it.
OpenSSL (3.2.0) was used the generate the localhost certs and the commands used
can be found in the `/certs` directory of the project.

### Directories

The server will require the following directories do be present during runtime
in order to function.

 - `/data`: data the server will storing during operation
 - `/storage`: user specific information such as uploaded files
 - `/storage/journals`: any uploaded data for a specific journal. the directory
   will contain the id of the journal with an additional subdirectory for
   storing the files attached to the journal.

These directories must be made manually as of right now and can be made in the
root of the project for convienience.

### Building

At the root of the project the following commands can be run to build all the
necessary files that are required for the server to run.

 - `cargo build` this will download all the necessary packges for the server
   and then create a debug build of the server
 - `npm install` downloads all the necessary packages for the frontend build
   process
 - `npm run build-tsx` builds the frontend Typescript and JSX (ReactJS) files
   for the user interface
 - `npm run build-css` builds the necessary CSS files for styling the user
   interface

### Database

There is a database configuration file in the `/db/postgres/init.sql` that
creates all the necessary information for the server. Here is a list of commands
that are used to setup the database

```
$ psql -U postgres
postgres# create database tj2;
postgres# \c tj2
postgres# \i db/postgres/init.sql
postgres# \q
```

this must be run at the root of the project in order for the path to the
`init.sql` to be properly resolved.

### Configuration

The server will require a configuration file at runtime to specify any relevant
information needed during operation. All relative paths will be resolved to the
parent directory of the config file being loaded.

```toml
# any additional configuration files to load befor processing this file
preload = [
    "./frontend/assets.config.toml"
]

# specifies the directory for the server to store information that is
# needed during operation
#
# defaults to "{CWD}/data"
data = "./data"

# specifies the directory for the server to store user information that
# is created during operation
#
# defaults to "{CWD}/storage"
storage = "./storage"

# the number of asynchronous threads that tokio will use for the thread
# pool.
#
# defaults to 1
thread_pool = 1

# the number of blocking threads that tokio will use for synchronous
# operations.
#
# defaults to 1
blocking_pool = 1

# the list of available listeners for the server to use
[[listeners]]
# the ipv4/ipv6 ip and port for the server to listen on
addr = "0.0.0.0:8080"

[[listeners]]
addr = "[::]:8443"

# additional tls information for the specific listener to use
[listeners.tls]
# the specified path of the private key to use
key = "./certs/localhost.ecdsa.key"
# the speicifed path of the certificate to use
cert = "./certs/localhost.ecdsa.crt"

# configuration information for connecting to the database
[db]
# the user for connecting to the database
#
# defaults to "postgres"
user = "postgres"

# the optional password for the user
#
# defaults to None
password = "password"

# the hostname of the database
#
# defaults to "localhost"
host = "localhost"

# the port the database is listening on
#
# defaults to 5432
port = 5432

# the name of the database to connect to
#
# defaults to "tj2"
dbname = "tj2"
```

The `/frontend` directory contains the UI information that the server will need
in order to serve frontend to a user so it must be specified in the `preload`
path list.

example that the server uses when running

```toml
preload = [
    "./frontend/assets.config.toml"
]

[[listeners]]
addr = "0.0.0.0:8080"

[[listeners]]
addr = "[::]:8443"

[listeners.tls]
key = "./certs/localhost.ecdsa.key"
cert = "./certs/localhost.ecdsa.crt"

[db]
password = "password"
```

## Running

Once all the setup work has been done to run the server just do

`cargo run -- ./server.config.toml -V info`

This will rebuild the server if any changes have been made in debug mode and
then run the program with the config file `./server.config.toml` and specify a
logging output of `info` which will output any `Error`, `Warn`, and `Info` logs
the server has.

The server will check for the existance of the `admin` user and create the
necessary permissions and roles for the `admin` user if not found. The default
password for the admin user will be `password`.
