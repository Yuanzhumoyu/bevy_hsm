use bevy::{
    color::palettes::css::NAVY,
    ecs::system::EntityCommands,
    feathers::{
        FeathersPlugins,
        controls::{ButtonProps, button},
        dark_theme::create_dark_theme,
        theme::UiTheme,
    },
    input_focus::tab_navigation::TabGroup,
    prelude::*,
    text::TextSpanAccess,
    ui_widgets::Activate,
};
use bevy_hsm::{prelude::*, system_registry};

#[derive(Resource, Default, Debug)]
struct Calculator {
    current_number: String,
    number_stack: Vec<f64>,
    operator_stack: Vec<char>,
    history_display: String,
    current_display: String,
    just_calculated: bool,
    input_buffer: Option<ButtonType>,
    parenthesis_count: u32,
}

impl Calculator {
    fn clear(&mut self) {
        self.current_number = "0".to_string();
        self.number_stack.clear();
        self.operator_stack.clear();
        self.current_display = "0".to_string();
        self.history_display = "".to_string();
        self.just_calculated = false;
        self.parenthesis_count = 0;
    }
}

// 按钮类型组件 - 用于标识UI按钮的功能
#[derive(Component, Message, Clone, Copy, Debug)]
enum ButtonType {
    Digit(u8),
    Operator(char),
    Command(Command),
}

// 命令类型 - 清除、等于、退格等操作
#[derive(Clone, Copy, Debug)]
enum Command {
    Clear,
    Equals,
    Backspace,
    ToggleSign,
}

#[derive(Component)]
struct Input;

#[derive(Component)]
struct Output;

#[derive(Resource, Default)]
struct StateEntityMap(std::collections::HashMap<&'static str, Entity>);

#[derive(Resource)]
struct FsmStates {
    operand: Entity,
    operator: Entity,
    left_parenthesis: Entity,
    right_parenthesis: Entity,
}

impl Default for FsmStates {
    fn default() -> Self {
        Self {
            operand: Entity::PLACEHOLDER,
            operator: Entity::PLACEHOLDER,
            left_parenthesis: Entity::PLACEHOLDER,
            right_parenthesis: Entity::PLACEHOLDER,
        }
    }
}

#[derive(Resource)]
struct HsmEntity(Entity);

// --- 辅助系统和函数 ---
fn debug_on_state(
    info: &str,
) -> impl Fn(In<ActionContext>, Query<&Name, Or<(With<HsmState>, With<FsmState>)>>) {
    move |context: In<ActionContext>, query: Query<&Name, Or<(With<HsmState>, With<FsmState>)>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

fn on_clear(_context: In<ActionContext>, mut calculator: ResMut<Calculator>) {
    calculator.clear();
}

fn on_equals(_context: In<ActionContext>, mut calculator: ResMut<Calculator>) {
    if !calculator.current_number.is_empty() {
        let val = calculator.current_number.parse::<f64>().unwrap_or(0.0);
        let Calculator {
            number_stack,
            current_number,
            history_display,
            ..
        } = calculator.as_mut();
        number_stack.push(val);
        history_display.push_str(&current_number);
        current_number.clear();
    }

    if calculator.parenthesis_count != 0 {
        for _ in 0..calculator.parenthesis_count {
            calculator.history_display.push(')');
        }
        calculator.parenthesis_count = 0;
    }

    while let Some(op) = calculator.operator_stack.pop() {
        if op == '(' {
            // Mismatched parenthesis, ignore.
            continue;
        }
        if calculator.number_stack.len() < 2 {
            break;
        }
        let b = calculator.number_stack.pop().unwrap();
        let a = calculator.number_stack.pop().unwrap();
        let result = calculate(a, b, op);
        calculator.number_stack.push(result);
    }

    if let Some(final_result) = calculator.number_stack.last().copied() {
        let full_expression = calculator.history_display.clone();
        calculator.history_display = format!("{} =", full_expression);
        calculator.current_display = final_result.to_string();
        calculator.current_number = final_result.to_string();
        calculator.just_calculated = true;
    }

    calculator.number_stack.clear();
    calculator.operator_stack.clear();
}

fn on_backspace(_context: In<ActionContext>, mut calculator: ResMut<Calculator>) {
    if calculator.just_calculated {
        return;
    }
    if !calculator.current_number.is_empty() && calculator.current_number != "0" {
        calculator.current_number.pop();
        if calculator.current_number.is_empty() {
            calculator.current_number = "0".to_string();
        }
    }
    calculator.current_display = calculator.current_number.clone();
}

fn on_toggle_sign(_context: In<ActionContext>, mut calculator: ResMut<Calculator>) {
    if !calculator.current_number.is_empty() && calculator.current_number != "0" {
        if calculator.current_number.starts_with('-') {
            calculator.current_number.remove(0);
        } else {
            calculator.current_number.insert(0, '-');
        }
        calculator.current_display = calculator.current_number.clone();
    }
}

fn hsm_exit_commands(
    In(contexts): In<Vec<ActionContext>>,
    mut commands: Commands,
) -> Option<Vec<ActionContext>> {
    for context in contexts {
        commands.trigger(HsmTrigger::to_super(context.state_machine));
    }
    None
}

fn precedence(op: char) -> i32 {
    match op {
        '+' | '-' => 1,
        '*' | '/' => 2,
        _ => 0, // For parentheses and other symbols
    }
}

fn on_enter_operand(In(_): In<ActionContext>, mut calculator: ResMut<Calculator>) {
    let Some(ButtonType::Digit(digit)) = calculator.input_buffer else {
        return;
    };

    if calculator.just_calculated {
        calculator.history_display.clear();
        calculator.current_number = String::new();
        calculator.just_calculated = false;
    }

    if digit == 10 {
        // Decimal point
        if !calculator.current_number.contains('.') {
            if calculator.current_number.is_empty() {
                calculator.current_number.push('0');
            }
            calculator.current_number.push('.');
        }
    } else if calculator.current_number == "0" {
        calculator.current_number = digit.to_string();
    } else {
        calculator.current_number.push_str(&digit.to_string());
    }
    calculator.current_display = calculator.current_number.clone();
}

fn on_enter_operator(
    In(_context): In<ActionContext>,
    mut calculator: ResMut<Calculator>,
    fsm: Single<&FsmStateMachine>,
    fsm_states: Res<FsmStates>,
) {
    let Some(ButtonType::Operator(op)) = calculator.input_buffer else {
        return;
    };

    // for +, -, *, /
    if let Some(prev) = fsm.history.get_at(1)
        && prev == fsm_states.operator
    {
        // This case handles replacing an operator (e.g., 5 + -)
        calculator.operator_stack.pop();
        let mut new_hist = calculator.history_display.trim_end().to_string();
        if !new_hist.is_empty() {
            new_hist.pop();
        }
        calculator.history_display = new_hist.trim_end().to_string();
    } else if !calculator.current_number.is_empty() {
        let Calculator {
            number_stack,
            history_display,
            current_number,
            just_calculated,
            ..
        } = calculator.as_mut();

        let current_val = current_number.parse::<f64>().unwrap_or(0.0);
        number_stack.push(current_val);
        if *just_calculated {
            history_display.clear();
            *just_calculated = false;
        }
        history_display.push_str(&current_number);
        calculator.current_number.clear();
    } else if calculator.just_calculated {
        let Calculator {
            history_display,
            current_display,
            just_calculated,
            ..
        } = calculator.as_mut();
        history_display.clear();
        history_display.push_str(&current_display);
        *just_calculated = false;
    }

    while let Some(&top_op) = calculator.operator_stack.last() {
        if top_op != '(' && precedence(op) <= precedence(top_op) {
            if calculator.number_stack.len() < 2 {
                break;
            }
            let b = calculator.number_stack.pop().unwrap();
            let a = calculator.number_stack.pop().unwrap();
            let op_to_apply = calculator.operator_stack.pop().unwrap();
            let result = calculate(a, b, op_to_apply);
            calculator.number_stack.push(result);
            calculator.current_display = result.to_string();
        } else {
            break;
        }
    }
    calculator.operator_stack.push(op);
    calculator.history_display.push(' ');
    calculator.history_display.push(op);
    calculator.history_display.push(' ');
}

fn on_left_parenthesis(
    In(_): In<ActionContext>,
    mut calculator: ResMut<Calculator>,
    fsm: Single<&FsmStateMachine>,
    fsm_states: Res<FsmStates>,
) {
    let Some(ButtonType::Operator('(')) = calculator.input_buffer else {
        return;
    };

    if calculator.just_calculated {
        calculator.clear();
    }

    if let Some(prev) = fsm.history.get_at(1)
        && prev == fsm_states.operand
    {
        if !calculator.current_number.is_empty() {
            let Calculator {
                number_stack,
                operator_stack,
                history_display,
                current_number,
                ..
            } = calculator.as_mut();

            let val = current_number.parse::<f64>().unwrap_or(0.0);
            number_stack.push(val);
            history_display.push_str(&current_number);
            current_number.clear();

            operator_stack.push('*');
            history_display.push_str(" * ");
        }
    }

    calculator.operator_stack.push('(');
    calculator.history_display.push('(');
    calculator.parenthesis_count += 1;
}

fn on_right_parenthesis(In(_context): In<ActionContext>, mut calculator: ResMut<Calculator>) {
    let Some(ButtonType::Operator(')')) = calculator.input_buffer else {
        return;
    };
    if calculator.parenthesis_count == 0 {
        return; // Ignore closing parenthesis if there are no open ones
    }
    if !calculator.current_number.is_empty() {
        let Calculator {
            number_stack,
            current_number,
            history_display,
            ..
        } = calculator.as_mut();
        let val = current_number.parse::<f64>().unwrap_or(0.0);
        number_stack.push(val);
        history_display.push_str(&current_number);
        current_number.clear();
    }

    while let Some(&top_op) = calculator.operator_stack.last() {
        if top_op == '(' {
            calculator.operator_stack.pop(); // Discard '('
            break;
        }
        if calculator.number_stack.len() < 2 {
            break;
        }
        let b = calculator.number_stack.pop().unwrap();
        let a = calculator.number_stack.pop().unwrap();
        let op_to_apply = calculator.operator_stack.pop().unwrap();
        let result = calculate(a, b, op_to_apply);
        calculator.number_stack.push(result);
        calculator.current_display = result.to_string();
    }
    calculator.history_display.push(')');
    calculator.parenthesis_count -= 1;
}

fn register_actions(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    system_registry!(<commands,action_registry>[
        "debug_on_enter" => debug_on_state("Entering state."),
        "debug_on_exit" => debug_on_state("Exiting state.")
    ]);
}

fn setup(mut commands: Commands, mut calculator: ResMut<Calculator>) {
    calculator.current_display = "0".to_string();
    calculator.history_display = "".to_string();

    let fsm_graph_id = commands
        .spawn(fsm_graph! {
            states: {
                #[state(on_enter=on_enter_operand)]: Operand,
                #[state(on_enter=on_enter_operator)]: Operator,
                #[state(on_enter=on_left_parenthesis)]: LeftParenthesis,
                #[state(on_enter=on_right_parenthesis)]: RightParenthesis,
            }
            transitions: {
                // From Operand state
                Operand => Operand, // e.g. 1 -> 12
                Operand => Operator, // e.g. 1 -> 1 +
                Operand => LeftParenthesis, // e.g. 1 -> 1 * (
                Operand => RightParenthesis, // e.g. (1+2) -> (1+2))

                // From Operator state
                Operator => Operator,
                Operator => Operand, // e.g. 1+ -> 1+2
                Operator => LeftParenthesis, // e.g. 1+ -> 1+(

                // From LeftParenthesis state
                LeftParenthesis => Operand, // e.g. ( -> (1
                LeftParenthesis => LeftParenthesis, // e.g. ( -> ((

                // From RightParenthesis state
                RightParenthesis => Operator, // e.g. (1) -> (1)+
                RightParenthesis => RightParenthesis, // e.g. (1)) -> (1)))
            }
            :|mut entity_commands: EntityCommands, states: &[Entity]| {
                entity_commands.commands_mut().insert_resource(FsmStates {
                    operand: states[0],
                    operator: states[1],
                    left_parenthesis: states[2],
                    right_parenthesis: states[3],
                });
            }
        })
        .id();

    commands.spawn(hsm! {
        #[state(on_enter="debug_on_enter", fsm_blueprint=FsmBlueprint::new(fsm_graph_id, 10))]
        :ProcessingInput(
            #[state(on_enter=on_clear, on_update="Update:hsm_exit_commands")]: Clear,
            #[state(on_enter=on_equals, on_update="Update:hsm_exit_commands")]: Equals,
            #[state(on_enter=on_backspace, on_update="Update:hsm_exit_commands")]: Backspace,
            #[state(on_enter=on_toggle_sign, on_update="Update:hsm_exit_commands")]: ToggleSign,
        ),
        StateLifecycle::default(),
        :|mut entity_commands:EntityCommands, ids:&[Entity]| {
            let state_machine_id = entity_commands.id();

            entity_commands.commands_mut().insert_resource(HsmEntity(state_machine_id));
            let mut map = StateEntityMap::default();
            map.0.insert("ProcessingInput", ids[0]);
            map.0.insert("Clear", ids[1]);
            map.0.insert("Equals", ids[2]);
            map.0.insert("Backspace", ids[3]);
            map.0.insert("ToggleSign", ids[4]);
            entity_commands.commands_mut().insert_resource(map);
        }
    });
}

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d::default());

    // 创建显示区域
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Px(50.0),
            right: Val::Px(50.0),
            ..Default::default()
        },
        Text::new("0"),
        TextFont::default().with_font_size(20.0),
        TextColor::WHITE,
        Name::new("Display"),
        Output,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(60.0),
            left: Val::Px(50.0),
            right: Val::Px(50.0),
            ..Default::default()
        },
        Text::new("0"),
        TextFont::default().with_font_size(40.0),
        TextColor::WHITE,
        Name::new("Display"),
        Input,
    ));

    let buttons = [
        vec![
            ("C", ButtonType::Command(Command::Clear)),
            ("⌫", ButtonType::Command(Command::Backspace)),
            ("(", ButtonType::Operator('(')),
            (")", ButtonType::Operator(')')),
        ],
        vec![
            ("7", ButtonType::Digit(7)),
            ("8", ButtonType::Digit(8)),
            ("9", ButtonType::Digit(9)),
            ("÷", ButtonType::Operator('/')),
        ],
        vec![
            ("4", ButtonType::Digit(4)),
            ("5", ButtonType::Digit(5)),
            ("6", ButtonType::Digit(6)),
            ("x", ButtonType::Operator('*')),
        ],
        vec![
            ("1", ButtonType::Digit(1)),
            ("2", ButtonType::Digit(2)),
            ("3", ButtonType::Digit(3)),
            ("-", ButtonType::Operator('-')),
        ],
        vec![
            ("+/-", ButtonType::Command(Command::ToggleSign)),
            ("0", ButtonType::Digit(0)),
            (".", ButtonType::Digit(10)),
            ("+", ButtonType::Operator('+')),
        ],
        vec![("=", ButtonType::Command(Command::Equals))],
    ];

    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::End,
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                border: px(5).into(),
                row_gap: px(5),
                padding: px(5).into(),
                align_items: AlignItems::Center,
                margin: px(50).into(),
                border_radius: BorderRadius::all(px(10)),
                ..Default::default()
            },
            BackgroundColor(NAVY.into()),
            BorderColor::all(Color::WHITE),
            children![
                Text::new("virtual keyboard"),
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.),
                        ..Default::default()
                    },
                    TabGroup::new(0),
                    Children::spawn(SpawnIter(buttons.into_iter().map(move |row| {
                        (
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(4.),
                                ..Default::default()
                            },
                            Children::spawn(SpawnIter(row.into_iter().map(
                                move |(key, button_type)| {
                                    (
                                    button(ButtonProps::default(), (), Spawn(Text::new(key))),
                                    bevy::ui_widgets::observe(
                                        move |_activate: On<Activate>,
                                              mut buttons_writer: MessageWriter<ButtonType>|
                                              -> Result {
                                            buttons_writer.write(button_type);
                                            Ok(())
                                        },
                                    ),
                                )
                                },
                            ))),
                        )
                    })))
                )
            ]
        )],
    ));
}

fn handle_button_message(
    mut commands: Commands,
    mut button_types: MessageReader<ButtonType>,
    mut calculator: ResMut<Calculator>,
    hsm_entity: Res<HsmEntity>,
    state_map: Res<StateEntityMap>,
    fsm_states: Res<FsmStates>,
) {
    if let Some(button_type) = button_types.read().last() {
        calculator.input_buffer = Some(*button_type);

        match button_type {
            ButtonType::Digit(_) | ButtonType::Operator(_) => {
                let target_fsm_state = match button_type {
                    ButtonType::Digit(_) => fsm_states.operand,
                    ButtonType::Operator(op) => match op {
                        '(' => fsm_states.left_parenthesis,
                        ')' => fsm_states.right_parenthesis,
                        _ => fsm_states.operator,
                    },
                    _ => unreachable!(),
                };
                commands.trigger(FsmTrigger::with_next(hsm_entity.0, target_fsm_state));
            }
            ButtonType::Command(cmd) => {
                let target_state_name = match cmd {
                    Command::Clear => "Clear",
                    Command::Equals => "Equals",
                    Command::Backspace => "Backspace",
                    Command::ToggleSign => "ToggleSign",
                };
                if let Some(target_state_entity) = state_map.0.get(target_state_name) {
                    commands.trigger(HsmTrigger::chain(hsm_entity.0, *target_state_entity));
                }
            }
        }
    }
}
fn update_display(
    calculator: Res<Calculator>,
    mut query: Query<(&mut Text, AnyOf<(&Input, &Output)>)>,
) {
    if calculator.is_changed() {
        for (mut text, selector) in query.iter_mut() {
            let is_input = selector.0.is_some();
            let is_output = selector.1.is_some();

            if is_input {
                *text.write_span() = calculator.current_display.clone();
            } else if is_output {
                *text.write_span() = calculator.history_display.clone();
            }
        }
    }
}

fn calculate(a: f64, b: f64, op: char) -> f64 {
    match op {
        '+' => a + b,
        '-' => a - b,
        '*' => a * b,
        '/' => {
            if b != 0.0 {
                a / b
            } else {
                f64::NAN
            }
        }
        _ => b,
    }
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(FeathersPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_message::<ButtonType>();
    app.init_resource::<Calculator>();
    app.init_resource::<StateEntityMap>();
    app.init_resource::<FsmStates>();
    app.insert_resource(UiTheme(create_dark_theme()));

    app.add_systems(Startup, (register_actions, setup, setup_ui).chain());
    app.add_systems(Update, (handle_button_message, update_display));

    app.add_action_system(Update, "hsm_exit_commands", hsm_exit_commands);
    app.run();
}
