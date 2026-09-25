# Exceptions

```{eval-rst}
.. automodule:: vstimd_client.exceptions
```

## Base classes

Catch these to handle a whole family at once. Every exception below is a
subclass of `VstimdError`, so `except VstimdError` catches anything the server
reports.

```{eval-rst}
.. autoexception:: vstimd_client.VstimdError
   :members: code, detail, command, handle

.. autoexception:: vstimd_client.StimulusError

.. autoexception:: vstimd_client.SceneConfigError
```

## Addressing a stimulus

```{eval-rst}
.. autoexception:: vstimd_client.HandleNotFoundError

.. autoexception:: vstimd_client.WrongStimulusTypeError

.. autoexception:: vstimd_client.WrongTargetError
```

## Command rejected

```{eval-rst}
.. autoexception:: vstimd_client.CreationFailedError

.. autoexception:: vstimd_client.InvalidArgumentError

.. autoexception:: vstimd_client.NotSupportedError

.. autoexception:: vstimd_client.NotReadyError

.. autoexception:: vstimd_client.UnknownServerError
```

## Scene configs

```{eval-rst}
.. autoexception:: vstimd_client.SceneConfigNotFoundError

.. autoexception:: vstimd_client.SceneConfigIoError

.. autoexception:: vstimd_client.SceneConfigFormatError

.. autoexception:: vstimd_client.SceneConfigVersionError

.. autoexception:: vstimd_client.SceneConfigAlreadyExistsError
```

## Client-side

```{eval-rst}
.. autoexception:: vstimd_client.ProtocolError
```
