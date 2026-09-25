# VstimdClient

```{eval-rst}
.. autoclass:: vstimd_client.VstimdClient
   :members:
   :undoc-members:
```

## ServerResponse

Returned by every mutation command.  Carries server-side timing metadata and
the error code (always `OK` — exceptions are raised on any other code).

```{eval-rst}
.. autoclass:: vstimd_client.ServerResponse
   :no-members:
   :no-undoc-members:

.. autoclass:: vstimd_client.ErrorCode
   :members:
```
