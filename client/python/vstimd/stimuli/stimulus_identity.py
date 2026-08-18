from dataclasses import dataclass

from vstimd._proto.vstimd.v1.stimuli.identity_pb2 import (
    StimulusIdentity as ProtoStimulusIdentity,
)


@dataclass
class StimulusIdentity:
    name: str

    @classmethod
    def from_proto(cls, proto: ProtoStimulusIdentity) -> 'StimulusIdentity':
        return cls(name=proto.name)

    def to_proto(self) -> ProtoStimulusIdentity:
        return ProtoStimulusIdentity(name=self.name)
