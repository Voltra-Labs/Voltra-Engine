(
    version: 2,
    entities: [
        (
            id: "019fe400-0000-7000-8000-0000000000a1",
            components: {
                "Camera2D": (size:5.0,priority:0,active:true),
                "Name": "Camera",
                "Transform2D": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)),
            },
        ),
        (
            id: "019fe400-0000-7000-8000-0000000000a2",
            components: {
                "Name": "Player",
                "Player": (speed:3.0),
                "Sprite2D": (color:(0.35,0.75,1.0,1.0),sort_order:1,texture:Some(Path("sprites/checker.png")),atlas:None,frame:0,region:None,flip_x:false,flip_y:false,pixels_per_unit:None),
                "Transform2D": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)),
            },
        ),
        (
            id: "019fe400-0000-7000-8000-0000000000a3",
            components: {
                "Name": "Title",
                "Text": (content:"{{title}}",font:None,size:0.5,pixels_per_em:48,color:(1.0,1.0,1.0,1.0),align:Center,sort_order:2),
                "Transform2D": (translation:(0.0,2.5),rotation:0.0,scale:(1.0,1.0)),
            },
        ),
    ],
)