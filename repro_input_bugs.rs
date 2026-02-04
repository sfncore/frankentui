#[cfg(test)]
mod tests {
    use ftui_core::event::{Event, KeyCode, KeyEvent, Modifiers, KeyEventKind};
    use ftui_core::input_parser::InputParser;
    use ftui_widgets::input::TextInput;

    #[test]
    fn test_alt_backspace_parsing() {
        let mut parser = InputParser::new();
        // ESC (0x1B) + DEL (0x7F) -> Should be Alt+Backspace
        let events = parser.parse(&[0x1B, 0x7F]);
        
        if events.is_empty() {
            println!("BUG: Alt+Backspace (0x1B 0x7F) was dropped!");
            panic!("Alt+Backspace dropped");
        }
        
        let event = &events[0];
        if let Event::Key(key) = event {
            assert_eq!(key.code, KeyCode::Backspace, "Expected Backspace code");
            assert!(key.modifiers.contains(Modifiers::ALT), "Expected Alt modifier");
        } else {
            panic!("Expected Key event, got {:?}", event);
        }
    }

    #[test]
    fn test_ctrl_w_behavior() {
        let mut input = TextInput::new().with_value("hello world");
        // Place cursor at end
        // "hello world|"
        
        // Ctrl+W event (standard Unix word erase)
        let event = Event::Key(KeyEvent::new(KeyCode::Char('w')).with_modifiers(Modifiers::CTRL));
        
        let handled = input.handle_event(&event);
        
        if !handled {
            println!("BUG: Ctrl+W was ignored by TextInput!");
            panic!("Ctrl+W ignored");
        }
        
        assert_eq!(input.value(), "hello ", "Ctrl+W should delete last word");
    }
}
