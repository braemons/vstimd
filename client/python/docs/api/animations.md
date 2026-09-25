# Animations

```{eval-rst}
.. data:: vstimd_client.AnimationHandle

   Opaque integer handle returned by every ``conn.animations.create_*`` call.
   Pass it to :meth:`~vstimd_client.AnimationClient.arm`,
   :meth:`~vstimd_client.AnimationClient.disarm`, and
   :meth:`~vstimd_client.AnimationClient.delete`.

.. autoclass:: vstimd_client.AnimationClient
   :members:
   :undoc-members:

.. autoclass:: vstimd_client.AnimationDetails
   :members:

.. autoclass:: vstimd_client.AnimationInfo
   :members:

.. autoclass:: vstimd_client.AnimationState
   :members:

.. autoclass:: vstimd_client.FinalAction
   :members:

.. autoclass:: vstimd_client.StartAction
   :members:

.. autoclass:: vstimd_client.VtlEdge
   :members:
```
