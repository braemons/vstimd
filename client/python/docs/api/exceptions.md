# Exceptions

```{eval-rst}
.. automodule:: vstimd.exceptions
```

## Base classes

Catch these to handle a whole family at once. Every exception below is a
subclass of `VstimdError`, so `except VstimdError` catches anything the server
reports.

```{eval-rst}
.. autoexception:: vstimd.VstimdError
   :members: code, detail, command, handle

.. autoexception:: vstimd.StimulusError

.. autoexception:: vstimd.ConfigError
```

## Addressing a stimulus

```{eval-rst}
.. autoexception:: vstimd.HandleNotFoundError

.. autoexception:: vstimd.WrongStimulusTypeError

.. autoexception:: vstimd.WrongTargetError
```

## Command rejected

```{eval-rst}
.. autoexception:: vstimd.CreationFailedError

.. autoexception:: vstimd.InvalidArgumentError

.. autoexception:: vstimd.NotSupportedError

.. autoexception:: vstimd.NotReadyError

.. autoexception:: vstimd.UnknownServerError
```

## Scene configs

```{eval-rst}
.. autoexception:: vstimd.ConfigNotFoundError

.. autoexception:: vstimd.ConfigIoError

.. autoexception:: vstimd.ConfigFormatError

.. autoexception:: vstimd.ConfigVersionError

.. autoexception:: vstimd.ConfigAlreadyExistsError
```

## Client-side

```{eval-rst}
.. autoexception:: vstimd.ProtocolError
```
