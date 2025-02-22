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
* Multiple Resultset queries (partially supported)
* Parameterized queries
* Client protocol < 4.1 (Not tested)
* Multifactor authentication
* Compression
* Packets > 16,777,215 Bytes
    * TODO: Terminate connection on large packet, or override max packet size in handshake.

## Supported:

* TLS (As an optional feature)
* Intercept insert queries by setting environment variable `INTERCEPT_INSERT=true`
    * Last Insert ID as part of query response will be returned by an atomic counter starting from 100.

## Roadmap:

* Intercept updates.
* Parameterized Queries