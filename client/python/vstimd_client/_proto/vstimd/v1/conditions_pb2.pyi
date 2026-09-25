from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ConditionAction(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CONDITION_ACTION_UNSPECIFIED: _ClassVar[ConditionAction]
    CONDITION_ACTION_RESET: _ClassVar[ConditionAction]
    CONDITION_ACTION_HOLD: _ClassVar[ConditionAction]
    CONDITION_ACTION_STOP: _ClassVar[ConditionAction]
CONDITION_ACTION_UNSPECIFIED: ConditionAction
CONDITION_ACTION_RESET: ConditionAction
CONDITION_ACTION_HOLD: ConditionAction
CONDITION_ACTION_STOP: ConditionAction

class Condition(_message.Message):
    __slots__ = ("index", "name")
    INDEX_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    index: int
    name: str
    def __init__(self, index: _Optional[int] = ..., name: _Optional[str] = ...) -> None: ...

class SetConditionRequest(_message.Message):
    __slots__ = ("index", "name")
    INDEX_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    index: int
    name: str
    def __init__(self, index: _Optional[int] = ..., name: _Optional[str] = ...) -> None: ...

class DeclareConditionsRequest(_message.Message):
    __slots__ = ("conditions",)
    CONDITIONS_FIELD_NUMBER: _ClassVar[int]
    conditions: _containers.RepeatedCompositeFieldContainer[Condition]
    def __init__(self, conditions: _Optional[_Iterable[_Union[Condition, _Mapping]]] = ...) -> None: ...

class ListConditionsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListConditionsResponse(_message.Message):
    __slots__ = ("conditions", "active_index", "active_name")
    CONDITIONS_FIELD_NUMBER: _ClassVar[int]
    ACTIVE_INDEX_FIELD_NUMBER: _ClassVar[int]
    ACTIVE_NAME_FIELD_NUMBER: _ClassVar[int]
    conditions: _containers.RepeatedCompositeFieldContainer[Condition]
    active_index: int
    active_name: str
    def __init__(self, conditions: _Optional[_Iterable[_Union[Condition, _Mapping]]] = ..., active_index: _Optional[int] = ..., active_name: _Optional[str] = ...) -> None: ...

class SetStimulusConditionsRequest(_message.Message):
    __slots__ = ("condition_indices",)
    CONDITION_INDICES_FIELD_NUMBER: _ClassVar[int]
    condition_indices: _containers.RepeatedScalarFieldContainer[int]
    def __init__(self, condition_indices: _Optional[_Iterable[int]] = ...) -> None: ...

class SetAnimationConditionsRequest(_message.Message):
    __slots__ = ("handle", "condition_indices", "condition_action")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    CONDITION_INDICES_FIELD_NUMBER: _ClassVar[int]
    CONDITION_ACTION_FIELD_NUMBER: _ClassVar[int]
    handle: int
    condition_indices: _containers.RepeatedScalarFieldContainer[int]
    condition_action: ConditionAction
    def __init__(self, handle: _Optional[int] = ..., condition_indices: _Optional[_Iterable[int]] = ..., condition_action: _Optional[_Union[ConditionAction, str]] = ...) -> None: ...
