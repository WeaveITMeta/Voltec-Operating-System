package dev.macrohard.qft;

/**
 * Exception thrown when a QFT file has an invalid format.
 */
public class QftFormatException extends Exception {
    
    public QftFormatException(String message) {
        super(message);
    }
    
    public QftFormatException(String message, Throwable cause) {
        super(message, cause);
    }
}
