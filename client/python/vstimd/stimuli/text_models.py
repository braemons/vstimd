from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum

from vstimd._proto.vstimd.v1.stimuli import text_pb2

from .color import Color
from .vec import Vec2


class LanguageStyle(Enum):
    LTR = "LTR"
    RTL = "RTL"
    ARABIC = "Arabic"


_LANGUAGE_STYLE_TO_PROTO: dict[LanguageStyle, text_pb2.LanguageStyle] = {
    LanguageStyle.LTR:    text_pb2.LANGUAGE_STYLE_LTR,
    LanguageStyle.RTL:    text_pb2.LANGUAGE_STYLE_RTL,
    LanguageStyle.ARABIC: text_pb2.LANGUAGE_STYLE_ARABIC,
}

_PROTO_TO_LANGUAGE_STYLE: dict[int, LanguageStyle] = {
    text_pb2.LANGUAGE_STYLE_LTR:    LanguageStyle.LTR,
    text_pb2.LANGUAGE_STYLE_RTL:    LanguageStyle.RTL,
    text_pb2.LANGUAGE_STYLE_ARABIC: LanguageStyle.ARABIC,
}


@dataclass
class TextParams:
    text: str = ""
    font: str = ""
    letter_height: float = 0.0
    size: Vec2 = field(default_factory=lambda: Vec2(0.0, 0.0))
    anchor: str = "center"
    text_color: Color = field(default_factory=lambda: Color(1.0, 1.0, 1.0, 1.0))
    fill_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0, 0.0))
    border_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0, 0.0))
    flip_horiz: bool = False
    language_style: LanguageStyle = LanguageStyle.LTR

    @classmethod
    def from_proto(cls, proto: text_pb2.TextParams) -> TextParams:
        return cls(
            text=proto.text,
            font=proto.font,
            letter_height=proto.letter_height,
            size=Vec2.from_proto(proto.size) if proto.HasField("size") else Vec2(0.0, 0.0),
            anchor=proto.anchor,
            text_color=Color.from_proto(proto.text_color)
            if proto.HasField("text_color")
            else Color(1.0, 1.0, 1.0, 1.0),
            fill_color=Color.from_proto(proto.fill_color)
            if proto.HasField("fill_color")
            else Color(0.0, 0.0, 0.0, 0.0),
            border_color=Color.from_proto(proto.border_color)
            if proto.HasField("border_color")
            else Color(0.0, 0.0, 0.0, 0.0),
            flip_horiz=proto.flip_horiz,
            language_style=_PROTO_TO_LANGUAGE_STYLE.get(proto.language_style, LanguageStyle.LTR),
        )
