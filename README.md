# MocksySQL

MocksySQL is a work-in-progress application with an as yet undecided scope. In broad strokes, it needs to achieve the
following:

* Log queries and intercept responses from the MySQL server.
* Return dummy responses for DML queries (INSERT, UPDATE, DELETE)
* Query interception for delta storage (Read data from primary database, persist data to secondary database).

## Good to haves

I hope to get these features implemented, but I may decide not to as well:

* Get drip-fed packet capture from production environment and return the same responses for specified queries from the
  proxy.
    * If a certain predetermined time passes after the query is received, perform read on primary database.

## Not supported:

* Binary Protocol
* TLS
    * As an extension, authentication methods that require encryption are not supported
* Multiple Resultset queries (partially supported)
* Client protocol < 4.1 (Not tested)
* Multifactor authentication
* Compression